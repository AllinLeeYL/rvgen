//! Serialize generated basic blocks and a resolved memory layout as RISC-V ELF.
//!
//! [`Elf`] starts from a [`MemoryLayout`] and is filled in steps: the program
//! goes into `.text` ([`Elf::add_code`]), [`ElfExtension`]s place whatever else
//! the image needs into free RAM (e.g. [`HostInterface`] adds
//! `.tohost`/`.fromhost`), and [`Elf::finish`] serializes it as ELF32/ELF64.

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail, ensure};

use crate::basicblock::BasicBlock;
use crate::memory::{MemoryLayout, MemoryRegion, Permissions, Section};
use crate::riscv::Xlen;
use crate::target::Target;

// ELF section and segment constants matching rvgen-go/elf.go
pub const SHF_WRITE: u64 = 1;
pub const SHF_ALLOC: u64 = 2;
pub const SHF_EXECINSTR: u64 = 4;
pub const MAX_ELF_SIZE: u64 = 1 << 30;

pub const SHT_PROGBITS: u32 = 1;
pub const SHT_SYMTAB: u32 = 2;
pub const SHT_STRTAB: u32 = 3;

pub const STT_OBJECT: u8 = 1;
pub const STT_FUNC: u8 = 2;

pub const PT_LOAD: u32 = 1;
pub const PF_X: u32 = 1;
pub const PF_W: u32 = 2;
pub const PF_R: u32 = 4;

/// Something that contributes sections or symbols to an image after the
/// program itself has been laid out.
pub trait ElfExtension {
    fn extend(&self, elf: &mut Elf) -> Result<()>;
}

/// Exports the HTIF mailboxes `tohost` and `fromhost` used by Spike and
/// riscv-tests style harnesses. Sections already present in the layout are
/// reused; missing ones are appended to free RAM.
pub struct HostInterface;

impl HostInterface {
    pub const SIZE: u64 = 8;
    pub const ALIGNMENT: u64 = 64;
}

impl ElfExtension for HostInterface {
    fn extend(&self, elf: &mut Elf) -> Result<()> {
        for (section, symbol) in [(".tohost", "tohost"), (".fromhost", "fromhost")] {
            let index = match elf.section_index(section) {
                Some(index) => index,
                None => elf.place(
                    section,
                    vec![0; Self::SIZE as usize],
                    Self::ALIGNMENT,
                    Permissions::RW,
                )?,
            };
            let existing = &elf.sections()[index];
            ensure!(
                existing.bytes.len() as u64 >= Self::SIZE
                    && existing.align >= Self::SIZE
                    && existing.flags & SHF_WRITE != 0,
                "{section} requires at least 8 writable bytes aligned to 8"
            );
            elf.add_symbol(ElfSymbol {
                name: symbol.into(),
                section: index,
                offset: 0,
                size: Self::SIZE,
                symbol_type: STT_OBJECT,
            })?;
        }
        Ok(())
    }
}

/// An executable image under construction: loadable sections kept in sync with
/// the memory layout, global symbols and an entry point. `.strtab`, `.symtab`
/// and `.shstrtab` are synthesized by [`Elf::finish`].
pub struct Elf<'a> {
    target: &'a Target,
    layout: MemoryLayout,
    sections: Vec<ElfSection>,
    symbols: Vec<ElfSymbol>,
    entry: Option<u64>,
}

impl<'a> Elf<'a> {
    /// Start from a validated layout, with every section zero-filled except .text,
    /// which [`Elf::add_code`] fills in.
    pub fn new(target: &'a Target, memory: &MemoryLayout) -> Result<Self> {
        memory.validate(&target.physical_memory, target.xlen)?;
        let mut elf = Self {
            target,
            layout: memory.clone(),
            sections: Vec::new(),
            symbols: Vec::new(),
            entry: None,
        };
        for section in &memory.sections {
            let bytes = if section.name == ".text" {
                Vec::new()
            } else {
                vec![0; usize::try_from(section.region.size)?]
            };
            elf.add_section(ElfSection::alloc(section, bytes))?;
        }
        Ok(elf)
    }

    pub fn target(&self) -> &Target {
        self.target
    }

    pub fn sections(&self) -> &[ElfSection] {
        &self.sections
    }

    pub fn section_index(&self, name: &str) -> Option<usize> {
        self.sections
            .iter()
            .position(|section| section.name == name)
    }

    /// Run an extension against this image.
    pub fn apply(&mut self, extension: impl ElfExtension) -> Result<()> {
        extension.extend(self)
    }

    /// Encode blocks in order into .text, make it the entry and export
    /// `_start` plus one symbol per labeled block. `.text` is placed at the
    /// lowest free RAM address when the layout lacks it.
    pub fn add_code(&mut self, bbs: &[BasicBlock]) -> Result<()> {
        ensure!(self.entry.is_none(), "code has already been added");
        let mut code = Vec::new();
        let mut labels = Vec::new();
        for bb in bbs {
            let offset = code.len() as u64;
            code.extend_from_slice(&bb.encode(self.target)?);
            if let Some(label) = &bb.label {
                labels.push((label, offset, code.len() as u64 - offset));
            }
        }
        let size = code.len() as u64;
        let alignment = self.target.instruction_alignment() as u64;
        let index = match self.section_index(".text") {
            Some(index) => {
                ensure!(
                    size == self.layout.get(".text")?.region.size,
                    "code size does not match .text allocation"
                );
                self.sections[index].bytes = code;
                index
            }
            None => {
                ensure!(!code.is_empty(), "no code to place in .text");
                self.place(".text", code, alignment, Permissions::RX)?
            }
        };
        let text = &self.sections[index];
        ensure!(text.flags & SHF_EXECINSTR != 0, ".text must be executable");
        ensure!(
            text.addr % alignment == 0,
            "entry address is not aligned for the target ISA"
        );
        self.entry = Some(text.addr);
        self.add_symbol(ElfSymbol {
            name: "_start".into(),
            section: index,
            offset: 0,
            size,
            symbol_type: STT_FUNC,
        })?;
        for (label, offset, size) in labels {
            self.add_symbol(ElfSymbol {
                name: label.clone(),
                section: index,
                offset,
                size,
                symbol_type: STT_FUNC,
            })?;
        }
        Ok(())
    }

    /// Allocate a new section at the lowest free, suitably aligned RAM address.
    pub fn place(
        &mut self,
        name: &str,
        bytes: Vec<u8>,
        alignment: u64,
        permissions: Permissions,
    ) -> Result<usize> {
        let size = bytes.len() as u64;
        ensure!(size > 0, "section {name} must not be empty");
        ensure!(alignment.is_power_of_two(), "invalid section alignment");
        let ram = &self.target.physical_memory;
        let ram_end = ram.end()?;
        let mut candidates: Vec<u64> = std::iter::once(ram.start)
            .chain(
                self.layout
                    .sections
                    .iter()
                    .filter_map(|section| section.region.end().ok()),
            )
            .collect();
        candidates.sort_unstable();
        let start = candidates
            .into_iter()
            .filter_map(|candidate| {
                let start = align_up(candidate, alignment).ok()?;
                let end = start.checked_add(size)?;
                let free = end <= ram_end
                    && self.layout.sections.iter().all(|other| {
                        end <= other.region.start || start >= other.region.start + other.region.size
                    });
                free.then_some(start)
            })
            .next()
            .ok_or_else(|| anyhow!("section {name} does not fit RAM"))?;
        let section = Section {
            name: name.into(),
            alignment,
            permissions,
            region: MemoryRegion {
                start,
                size,
                permissions,
            },
            private: false,
        };
        let index = self.add_section(ElfSection::alloc(&section, bytes))?;
        self.layout.add(section);
        Ok(index)
    }

    pub fn add_symbol(&mut self, symbol: ElfSymbol) -> Result<()> {
        ensure!(
            valid_elf_name(&symbol.name)
                && symbol.symbol_type <= 15
                && self.symbols.iter().all(|s| s.name != symbol.name),
            "invalid or duplicate ELF symbol {:?}",
            symbol.name
        );
        let section = self
            .sections
            .get(symbol.section)
            .ok_or_else(|| anyhow!("invalid symbol section"))?;
        ensure!(
            add64(symbol.offset, symbol.size)? <= section.bytes.len() as u64,
            "symbol {} lies outside its section",
            symbol.name
        );
        self.symbols.push(symbol);
        Ok(())
    }

    /// Sections are only added through [`Elf::new`] and [`Elf::place`] so the
    /// image never drifts from the memory layout.
    fn add_section(&mut self, section: ElfSection) -> Result<usize> {
        ensure!(
            valid_elf_name(&section.name)
                && !matches!(section.name.as_str(), ".strtab" | ".symtab" | ".shstrtab")
                && self.section_index(&section.name).is_none(),
            "invalid or duplicate section name: {:?}",
            section.name
        );
        self.sections.push(section);
        Ok(self.sections.len() - 1)
    }

    /// Serialize with string and symbol tables appended after the user sections.
    pub fn finish(self) -> Result<Vec<u8>> {
        let entry = self
            .entry
            .ok_or_else(|| anyhow!("no code has been added"))?;
        self.layout
            .validate(&self.target.physical_memory, self.target.xlen)?;
        let is_64bit = self.target.xlen == Xlen::X64;
        let mut sections = self.sections;
        let mut strings = vec![0u8];
        let entry_size = if is_64bit { 24 } else { 16 };
        let mut table = ElfBytes::new(entry_size, is_64bit);
        table.data.resize(entry_size, 0); // STN_UNDEF entry

        for s in &self.symbols {
            if s.section + 1 > u16::MAX as usize {
                bail!("invalid symbol section");
            }
            let value = add64(sections[s.section].addr, s.offset)?;
            if strings.len() as u64 > u32::MAX as u64 {
                bail!("symbol string table too large");
            }
            table.u32(strings.len() as u32);
            strings.extend_from_slice(s.name.as_bytes());
            strings.push(0);
            if is_64bit {
                table.data.push(0x10 | s.symbol_type);
                table.data.push(0);
                table.u16((s.section + 1) as u16);
                table.u64(value);
                table.u64(s.size);
            } else {
                table.word(value)?;
                table.word(s.size)?;
                table.data.push(0x10 | s.symbol_type);
                table.data.push(0);
                table.u16((s.section + 1) as u16);
            }
        }

        let string_index = (sections.len() + 1) as u32;
        sections.push(ElfSection::metadata(".strtab", strings, SHT_STRTAB, 1));
        sections.push(ElfSection {
            link: string_index,
            info: 1,
            entsize: entry_size as u64,
            ..ElfSection::metadata(
                ".symtab",
                table.data,
                SHT_SYMTAB,
                if is_64bit { 8 } else { 4 },
            )
        });

        serialize(&sections, is_64bit, entry)
    }
}

/// Writes an already encoded ELF image to disk.
pub fn write_executable(bytes: &[u8], path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    std::fs::write(path, bytes)
        .with_context(|| format!("failed to write ELF to {}", path.display()))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ElfSection {
    pub name: String,
    pub bytes: Vec<u8>,
    pub addr: u64,
    pub flags: u64,
    pub align: u64,
    pub section_type: u32,
    pub link: u32,
    pub info: u32,
    pub entsize: u64,
}

impl ElfSection {
    /// A loadable section backing a memory-layout allocation.
    fn alloc(section: &Section, bytes: Vec<u8>) -> Self {
        Self {
            name: section.name.clone(),
            bytes,
            addr: section.region.start,
            flags: section_flags(section.permissions),
            align: section.alignment,
            section_type: SHT_PROGBITS,
            ..Default::default()
        }
    }

    fn metadata(name: &str, bytes: Vec<u8>, section_type: u32, align: u64) -> Self {
        Self {
            name: name.into(),
            bytes,
            align,
            section_type,
            ..Default::default()
        }
    }
}

/// A global symbol defined at an offset within a section (zero-based index).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElfSymbol {
    pub name: String,
    pub section: usize,
    pub offset: u64,
    pub size: u64,
    /// ELF symbol type (e.g. STT_OBJECT = 1, STT_FUNC = 2).
    pub symbol_type: u8,
}

fn section_flags(permissions: Permissions) -> u64 {
    SHF_ALLOC
        | if permissions.write { SHF_WRITE } else { 0 }
        | if permissions.execute {
            SHF_EXECINSTR
        } else {
            0
        }
}

pub fn section_flags_to_program_flags(flags: u64) -> u32 {
    let mut p = PF_R;
    if flags & SHF_WRITE != 0 {
        p |= PF_W;
    }
    if flags & SHF_EXECINSTR != 0 {
        p |= PF_X;
    }
    p
}

fn add64(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b)
        .ok_or_else(|| anyhow!("ELF layout exceeds 64 bits"))
}

fn align_up(n: u64, alignment: u64) -> Result<u64> {
    let alignment = alignment.max(1);
    add64(n, (alignment - n % alignment) % alignment)
}

fn valid_elf_name(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b != 0 && b < 128)
}

struct ElfBytes {
    data: Vec<u8>,
    is64: bool,
}

impl ElfBytes {
    fn new(capacity: usize, is64: bool) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            is64,
        }
    }

    fn u16(&mut self, v: u16) {
        self.data.extend_from_slice(&v.to_le_bytes());
    }

    fn u32(&mut self, v: u32) {
        self.data.extend_from_slice(&v.to_le_bytes());
    }

    fn u64(&mut self, v: u64) {
        self.data.extend_from_slice(&v.to_le_bytes());
    }

    fn word(&mut self, v: u64) -> Result<()> {
        if self.is64 {
            self.u64(v);
        } else {
            let v32 = u32::try_from(v).map_err(|_| anyhow!("value does not fit ELF32"))?;
            self.u32(v32);
        }
        Ok(())
    }

    fn pad(&mut self, n: u64) {
        let target = n as usize;
        if self.data.len() < target {
            self.data.resize(target, 0);
        }
    }
}

/// Builds an ELF32 or ELF64 executable image from the given sections.
fn serialize(sections: &[ElfSection], is_64bit: bool, start: u64) -> Result<Vec<u8>> {
    if sections.len() + 2 >= 0xff00 {
        bail!("too many ELF sections");
    }
    let (ehsize, phsize, shsize) = if is_64bit {
        (64u64, 56u64, 64u64)
    } else {
        (52u64, 32u64, 40u64)
    };

    let phnum = sections.iter().filter(|s| s.flags & SHF_ALLOC != 0).count() as u64;

    let mut names = HashSet::new();
    let mut shstrtab = vec![0u8];
    let mut name_offsets = Vec::with_capacity(sections.len() + 1);
    let mut offsets = Vec::with_capacity(sections.len() + 1);
    let mut offset = ehsize + phnum * phsize;

    for s in sections {
        if !valid_elf_name(&s.name) || s.name == ".shstrtab" || !names.insert(&s.name) {
            bail!("invalid or duplicate section name: {:?}", s.name);
        }
        if s.align > 1 && !s.align.is_power_of_two() {
            bail!("section {} alignment must be a power of two", s.name);
        }
        let end = add64(s.addr, s.bytes.len() as u64)?;
        if !is_64bit && (s.addr > u32::MAX as u64 || end > 1u64 << 32) {
            bail!("section address does not fit ELF32");
        }
        if shstrtab.len() as u64 > u32::MAX as u64 {
            bail!("section name table too large");
        }
        name_offsets.push(shstrtab.len() as u32);
        shstrtab.extend_from_slice(s.name.as_bytes());
        shstrtab.push(0);
        offset = align_up(offset, s.align)?;
        offsets.push(offset);
        offset = add64(offset, s.bytes.len() as u64)?;
    }

    name_offsets.push(shstrtab.len() as u32);
    shstrtab.extend_from_slice(b".shstrtab\0");
    offsets.push(offset);
    let shoff = align_up(add64(offset, shstrtab.len() as u64)?, 8)?;
    let file_size = add64(shoff, (sections.len() as u64 + 2) * shsize)?;

    if !is_64bit && (file_size > u32::MAX as u64 || start > u32::MAX as u64) {
        bail!("ELF layout or entry address does not fit ELF32");
    }
    if file_size > MAX_ELF_SIZE {
        bail!("ELF image exceeds {MAX_ELF_SIZE}-byte in-memory limit");
    }

    let mut b = ElfBytes::new(file_size as usize, is_64bit);
    let class = if is_64bit { 2u8 } else { 1u8 };
    b.data
        .extend_from_slice(&[0x7f, b'E', b'L', b'F', class, 1, 1, 0]);
    b.pad(16);
    b.u16(2); // ET_EXEC
    b.u16(0xf3); // EM_RISCV
    b.u32(1); // EV_CURRENT
    b.word(start)?;
    b.word(ehsize)?;
    b.word(shoff)?;
    b.u32(0); // e_flags
    for v in [
        ehsize as u16,
        phsize as u16,
        phnum as u16,
        shsize as u16,
        (sections.len() + 2) as u16,
        (sections.len() + 1) as u16,
    ] {
        b.u16(v);
    }

    for (i, s) in sections.iter().enumerate() {
        if s.flags & SHF_ALLOC == 0 {
            continue;
        }
        let flags = section_flags_to_program_flags(s.flags);
        b.u32(PT_LOAD);
        if is_64bit {
            b.u32(flags);
        }
        for v in [
            offsets[i],
            s.addr,
            s.addr,
            s.bytes.len() as u64,
            s.bytes.len() as u64,
        ] {
            b.word(v)?;
        }
        if !is_64bit {
            b.u32(flags);
        }
        b.word(s.align)?;
    }

    for (i, s) in sections.iter().enumerate() {
        b.pad(offsets[i]);
        b.data.extend_from_slice(&s.bytes);
    }
    b.data.extend_from_slice(&shstrtab);
    b.pad(shoff + shsize); // Section 0 is SHT_NULL (all zeros)

    let shstrtab_section = ElfSection::metadata(".shstrtab", shstrtab, SHT_STRTAB, 1);

    for (i, s) in sections
        .iter()
        .chain(std::iter::once(&shstrtab_section))
        .enumerate()
    {
        b.u32(name_offsets[i]);
        b.u32(s.section_type);
        for v in [s.flags, s.addr, offsets[i], s.bytes.len() as u64] {
            b.word(v)?;
        }
        b.u32(s.link);
        b.u32(s.info);
        b.word(s.align)?;
        b.word(s.entsize)?;
    }

    debug_assert_eq!(b.data.len(), file_size as usize);
    Ok(b.data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::riscv::{Extension, Instruction, PrivilegeLevel};

    fn target(xlen: Xlen, compressed: bool) -> Target {
        let mut extensions = vec![Extension::I, Extension::Zicsr];
        if compressed {
            extensions.push(Extension::C);
        }
        Target::new(
            xlen,
            extensions,
            [PrivilegeLevel::Machine, PrivilegeLevel::Supervisor, PrivilegeLevel::User],
            HashSet::new(),
            1,
            2,
            MemoryRegion {
                start: 0x8000_0000,
                size: 0x10000,
                permissions: Permissions::RWX,
            },
        )
        .unwrap()
    }

    fn section(name: &str, start: u64, size: u64, permissions: Permissions) -> Section {
        Section {
            name: name.into(),
            alignment: 4,
            permissions,
            region: MemoryRegion {
                start,
                size,
                permissions,
            },
            private: false,
        }
    }

    /// The standard pipeline: code, then the host interface.
    fn encode(target: &Target, bbs: &[BasicBlock], memory: &MemoryLayout) -> Result<Vec<u8>> {
        let mut elf = Elf::new(target, memory)?;
        elf.add_code(bbs)?;
        elf.apply(HostInterface)?;
        elf.finish()
    }

    fn nop_block() -> BasicBlock {
        BasicBlock {
            instrs: vec![Instruction::nop()],
            ..Default::default()
        }
    }

    /// Minimal ELF reader so tests check meaning rather than raw offsets.
    struct Parsed {
        entry: u64,
        segments: Vec<(u32, u32, u64, Vec<u8>)>, // flags, vaddr, contents
        sections: Vec<(String, u64, u64, Vec<u8>)>, // name, flags, addr, contents
        symbols: Vec<(String, u8, u16, u64, u64)>, // name, info, shndx, value, size
    }

    impl Parsed {
        fn new(bytes: &[u8]) -> Self {
            let is64 = bytes[4] == 2;
            let w = if is64 { 8 } else { 4 };
            let n = |offset: usize, width: usize| {
                let mut value = [0; 8];
                value[..width].copy_from_slice(&bytes[offset..offset + width]);
                u64::from_le_bytes(value) as usize
            };
            let (phoff, shoff) = (n(24 + w, w), n(24 + 2 * w, w));
            let h = if is64 { 52 } else { 40 };
            let (phentsize, phnum) = (n(h + 2, 2), n(h + 4, 2));
            let (shentsize, shnum, shstrndx) = (n(h + 6, 2), n(h + 8, 2), n(h + 10, 2));
            let segments = (0..phnum)
                .map(|i| {
                    let p = phoff + i * phentsize;
                    let (flags, off, vaddr, size) = if is64 {
                        (n(p + 4, 4), n(p + 8, 8), n(p + 16, 8), n(p + 32, 8))
                    } else {
                        (n(p + 24, 4), n(p + 4, 4), n(p + 8, 4), n(p + 16, 4))
                    };
                    let (ty, flags) = (n(p, 4) as u32, flags as u32);
                    (ty, flags, vaddr as u64, bytes[off..off + size].to_vec())
                })
                .collect();
            let headers: Vec<_> = (0..shnum)
                .map(|i| {
                    let s = shoff + i * shentsize;
                    let at = |k: usize| n(s + 8 + k * w, w);
                    (n(s, 4), at(0) as u64, at(1) as u64, at(2), at(3))
                })
                .collect();
            let (_, _, _, stroff, _) = headers[shstrndx];
            let cstr = |offset: usize| {
                let end = bytes[offset..].iter().position(|&b| b == 0).unwrap();
                String::from_utf8(bytes[offset..offset + end].to_vec()).unwrap()
            };
            let sections: Vec<_> = headers[1..]
                .iter()
                .map(|&(name, flags, addr, off, size)| {
                    (
                        cstr(stroff + name),
                        flags,
                        addr,
                        bytes[off..off + size].to_vec(),
                    )
                })
                .collect();
            let symtab = sections.iter().position(|s| s.0 == ".symtab").unwrap() + 1;
            let strtab = headers[sections.iter().position(|s| s.0 == ".strtab").unwrap() + 1].3;
            let (_, _, _, symoff, symsize) = headers[symtab];
            let entsize = if is64 { 24 } else { 16 };
            let symbols = (1..symsize / entsize)
                .map(|i| {
                    let s = symoff + i * entsize;
                    let (info, shndx, value, size) = if is64 {
                        (n(s + 4, 1), n(s + 6, 2), n(s + 8, 8), n(s + 16, 8))
                    } else {
                        (n(s + 12, 1), n(s + 14, 2), n(s + 4, 4), n(s + 8, 4))
                    };
                    (
                        cstr(strtab + n(s, 4)),
                        info as u8,
                        shndx as u16,
                        value as u64,
                        size as u64,
                    )
                })
                .collect();
            Self {
                entry: n(24, w) as u64,
                segments,
                sections,
                symbols,
            }
        }

        fn section(&self, name: &str) -> &(String, u64, u64, Vec<u8>) {
            self.sections.iter().find(|s| s.0 == name).unwrap()
        }

        fn symbol(&self, name: &str) -> &(String, u8, u16, u64, u64) {
            self.symbols.iter().find(|s| s.0 == name).unwrap()
        }
    }

    #[test]
    fn encodes_supplied_blocks_and_sections_for_both_xlens() {
        for xlen in [Xlen::X32, Xlen::X64] {
            let target = target(xlen, true);
            // Deliberately put .text second and away from the physical RAM base.
            let memory = MemoryLayout {
                sections: vec![
                    section(".data", 0x8000_1000, 16, Permissions::RW),
                    section(".text", 0x8000_0100, 6, Permissions::RX),
                ],
            };
            let blocks = [
                nop_block(),
                BasicBlock {
                    instrs: vec![Instruction::cnop()],
                    label: Some("_exit".into()),
                    ..Default::default()
                },
            ];
            let bytes = encode(&target, &blocks, &memory).unwrap();
            assert_eq!(&bytes[..4], b"\x7fELF");
            assert_eq!(bytes[4], if xlen == Xlen::X64 { 2 } else { 1 });
            let elf = Parsed::new(&bytes);
            assert_eq!(elf.entry, 0x8000_0100);
            let names: Vec<_> = elf.sections.iter().map(|s| s.0.as_str()).collect();
            assert_eq!(
                names,
                [
                    ".data",
                    ".text",
                    ".tohost",
                    ".fromhost",
                    ".strtab",
                    ".symtab",
                    ".shstrtab"
                ]
            );
            assert_eq!(elf.segments.len(), 4);
            assert_eq!(elf.segments[0], (PT_LOAD, 6, 0x8000_1000, vec![0; 16]));
            assert_eq!(
                elf.segments[1],
                (PT_LOAD, 5, 0x8000_0100, vec![0x13, 0, 0, 0, 1, 0])
            );
            // _start and labels refer to .text (section index 2) even when it is not first.
            assert_eq!(
                elf.symbol("_start"),
                &("_start".to_string(), 0x12, 2, 0x8000_0100, 6)
            );
            assert_eq!(
                elf.symbol("_exit"),
                &("_exit".to_string(), 0x12, 2, 0x8000_0104, 2)
            );
        }
    }

    #[test]
    fn appends_tohost_and_fromhost_for_both_xlens() {
        for xlen in [Xlen::X32, Xlen::X64] {
            let target = target(xlen, false);
            let bytes = encode(&target, &[nop_block()], &MemoryLayout::default()).unwrap();
            let elf = Parsed::new(&bytes);
            // .text is placed at the RAM base, the mailboxes after it.
            assert_eq!(elf.entry, 0x8000_0000);
            assert_eq!(elf.section(".text").2, 0x8000_0000);
            for (index, (name, address)) in [("tohost", 0x8000_0040), ("fromhost", 0x8000_0080)]
                .into_iter()
                .enumerate()
            {
                let section = elf.section(&format!(".{name}"));
                assert_eq!(section.1, SHF_ALLOC | SHF_WRITE);
                assert_eq!((section.2, &section.3[..]), (address, &[0u8; 8][..]));
                assert_eq!(
                    elf.symbol(name),
                    &(name.to_string(), 0x11, index as u16 + 2, address, 8)
                );
            }
        }
    }

    #[test]
    fn reuses_mailboxes_already_in_the_layout() {
        let target = target(Xlen::X64, false);
        let mut memory = MemoryLayout::default();
        for name in [".fromhost", ".tohost"] {
            memory
                .reserve(&target.physical_memory, name, 8, 64, Permissions::RW)
                .unwrap();
        }
        let bytes = encode(&target, &[nop_block()], &memory).unwrap();
        let elf = Parsed::new(&bytes);
        assert_eq!(elf.sections.iter().filter(|s| s.0 == ".tohost").count(), 1);
        for (index, name) in ["fromhost", "tohost"].into_iter().enumerate() {
            let symbol = elf.symbol(name);
            assert_eq!(symbol.2, index as u16 + 1);
            assert_eq!(symbol.3, memory.sections[index].region.start);
        }
        assert_eq!(elf.entry, 0x8000_0000);
    }

    #[test]
    fn runs_custom_extensions_after_the_defaults() {
        struct Signature;
        impl ElfExtension for Signature {
            fn extend(&self, elf: &mut Elf) -> Result<()> {
                ensure!(elf.section_index(".tohost").is_some());
                let index = elf.place(".signature", vec![0xaa; 16], 16, Permissions::RW)?;
                elf.add_symbol(ElfSymbol {
                    name: "begin_signature".into(),
                    section: index,
                    offset: 0,
                    size: 16,
                    symbol_type: STT_OBJECT,
                })
            }
        }
        let target = target(Xlen::X32, false);
        let mut elf = Elf::new(&target, &MemoryLayout::default()).unwrap();
        elf.add_code(&[nop_block()]).unwrap();
        elf.apply(HostInterface).unwrap();
        elf.apply(Signature).unwrap();
        let bytes = elf.finish().unwrap();
        let elf = Parsed::new(&bytes);
        assert_eq!(elf.section(".signature").2, 0x8000_0010);
        assert_eq!(elf.section(".signature").3, vec![0xaa; 16]);
        assert_eq!(elf.symbol("begin_signature").3, 0x8000_0010);
    }

    #[test]
    fn rejects_mismatched_layout_and_unsupported_instructions() {
        let target = target(Xlen::X64, false);
        let blocks = [nop_block()];
        assert!(
            encode(&target, &[], &MemoryLayout::default())
                .unwrap_err()
                .to_string()
                .contains("no code")
        );
        let mut memory = MemoryLayout {
            sections: vec![section(".text", 0x8000_0000, 8, Permissions::RX)],
        };
        assert!(
            encode(&target, &blocks, &memory)
                .unwrap_err()
                .to_string()
                .contains("code size")
        );
        memory.sections[0].region.size = 4;
        memory.add(section(".data", 0x8000_0000, 4, Permissions::RW));
        assert!(
            encode(&target, &blocks, &memory)
                .unwrap_err()
                .to_string()
                .contains("overlap")
        );
        memory.sections.pop();
        memory.sections[0].permissions = Permissions::RW;
        memory.sections[0].region.permissions = Permissions::RW;
        assert!(
            encode(&target, &blocks, &memory)
                .unwrap_err()
                .to_string()
                .contains("executable")
        );
        memory.sections[0].permissions = Permissions::RX;
        memory.sections[0].region.permissions = Permissions::RX;
        memory.sections[0].region.size = 2;
        let compressed = [BasicBlock {
            instrs: vec![Instruction::cnop()],
            ..Default::default()
        }];
        assert!(
            encode(&target, &compressed, &memory)
                .unwrap_err()
                .to_string()
                .contains("not enabled")
        );
        memory.add(section(".tohost", 0x8000_0010, 4, Permissions::RW));
        memory.sections[0].region.size = 4;
        assert!(
            encode(&target, &blocks, &memory)
                .unwrap_err()
                .to_string()
                .contains("8 writable bytes")
        );
    }
}
