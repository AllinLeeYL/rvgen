use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result, bail};
use rand::{Rng, RngExt};

pub fn cut_cake_randomly(
    total: usize,
    smallest_cake: Option<usize>,
    biggest_cake: Option<usize>,
    rng: &mut (impl Rng + ?Sized),
) -> Vec<usize> {
    let smallest_cake = smallest_cake.unwrap_or(1);
    let biggest_cake = biggest_cake.unwrap_or(32);

    assert!(
        smallest_cake > 0 && smallest_cake <= biggest_cake,
        "cake size must satisfy 1 <= smallest <= biggest"
    );

    let mut remaining = total;
    let mut sizes = Vec::new();

    while remaining > 0 {
        let lower = smallest_cake.min(remaining);
        let upper = biggest_cake.min(remaining);
        let size = rng.random_range(lower..=upper);

        sizes.push(size);
        remaining -= size;
    }

    sizes
}

// ELF section and segment constants matching rvgen-go/elf.go
pub const SHF_WRITE: u64 = 1;
pub const SHF_ALLOC: u64 = 2;
pub const SHF_EXECINSTR: u64 = 4;
pub const MAX_ELF_SIZE: u64 = 1 << 30;

pub const PT_LOAD: u32 = 1;
pub const PF_X: u32 = 1;
pub const PF_W: u32 = 2;
pub const PF_R: u32 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub fn new(name: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            name: name.into(),
            bytes: bytes.into(),
            addr: 0,
            flags: SHF_ALLOC | SHF_EXECINSTR,
            align: 4,
            section_type: 1, // SHT_PROGBITS
            link: 0,
            info: 0,
            entsize: 0,
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

fn add64(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b)
        .ok_or_else(|| anyhow::anyhow!("ELF layout exceeds 64 bits"))
}

fn align_up(n: u64, alignment: u64) -> Result<u64> {
    let alignment = alignment.max(1);
    add64(n, (alignment - n % alignment) % alignment)
}

fn valid_elf_name(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b != 0 && b < 128)
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
            let v32 = u32::try_from(v).map_err(|_| anyhow::anyhow!("value does not fit ELF32"))?;
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

/// Builds an ELF image with string table and symbol table sections.
pub fn build_elf_with_symbols(
    sections: &[ElfSection],
    symbols: &[ElfSymbol],
    is_64bit: bool,
    start: u64,
) -> Result<Vec<u8>> {
    let mut sections = sections.to_vec();
    let mut strings = vec![0u8];
    let entry_size = if is_64bit { 24 } else { 16 };
    let mut table = ElfBytes::new(entry_size, is_64bit);
    table.data.resize(entry_size, 0); // STN_UNDEF entry

    let mut names = HashSet::new();
    for s in symbols {
        if !valid_elf_name(&s.name) || !names.insert(&s.name) || s.symbol_type > 15 {
            bail!("invalid or duplicate ELF symbol");
        }
        if s.section >= sections.len() || s.section + 1 > u16::MAX as usize {
            bail!("invalid symbol section");
        }
        let section = &sections[s.section];
        let end = add64(s.offset, s.size)?;
        if end > section.bytes.len() as u64 {
            bail!("symbol lies outside its section");
        }
        let value = add64(section.addr, s.offset)?;
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
    let mut strtab = ElfSection::new(".strtab", strings);
    strtab.flags = 0;
    strtab.align = 1;
    strtab.section_type = 3; // SHT_STRTAB
    sections.push(strtab);

    let mut symtab = ElfSection::new(".symtab", table.data);
    symtab.flags = 0;
    symtab.align = if is_64bit { 8 } else { 4 };
    symtab.section_type = 2; // SHT_SYMTAB
    symtab.link = string_index;
    symtab.info = 1;
    symtab.entsize = entry_size as u64;
    sections.push(symtab);

    build_elf(&sections, is_64bit, start)
}

/// Builds an ELF32 or ELF64 executable image from the given sections.
pub fn build_elf(sections: &[ElfSection], is_64bit: bool, start: u64) -> Result<Vec<u8>> {
    if sections.len() + 2 >= 0xff00 {
        bail!("too many ELF sections");
    }
    let (ehsize, phsize, shsize) = if is_64bit {
        (64u64, 56u64, 64u64)
    } else {
        (52u64, 32u64, 40u64)
    };

    let phnum = sections
        .iter()
        .filter(|s| s.flags & SHF_ALLOC != 0)
        .count() as u64;

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
    b.data.extend_from_slice(&[0x7f, b'E', b'L', b'F', class, 1, 1, 0]);
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

    let shstrtab_section = ElfSection {
        name: ".shstrtab".to_string(),
        bytes: shstrtab,
        addr: 0,
        flags: 0,
        align: 1,
        section_type: 3, // SHT_STRTAB
        link: 0,
        info: 0,
        entsize: 0,
    };

    for (i, s) in sections.iter().chain(std::iter::once(&shstrtab_section)).enumerate() {
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

/// Writes raw code as a little-endian RISC-V ELF64 executable.
/// The code is loaded at, and execution starts at, `0x8000_0000`.
/// Compressed instructions are permitted; the bytes are preserved verbatim.
#[allow(dead_code)]
pub fn write_elf(bytes: &[u8], path: impl AsRef<Path>) -> Result<()> {
    const TEXT_ADDRESS: u64 = 0x8000_0000;
    let mut text = ElfSection::new(".text", bytes.to_vec());
    text.addr = TEXT_ADDRESS;
    text.align = 4;

    let symbols = [ElfSymbol {
        name: "_start".to_string(),
        section: 0,
        offset: 0,
        size: bytes.len() as u64,
        symbol_type: 2, // STT_FUNC
    }];

    let elf_bytes = build_elf_with_symbols(&[text], &symbols, true, TEXT_ADDRESS)?;
    let p = path.as_ref();
    std::fs::write(p, elf_bytes)
        .with_context(|| format!("failed to write ELF to {}", p.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_elf_sections_and_validation() {
        let mut code = ElfSection::new(".text", vec![0x13, 0, 0, 0]);
        code.addr = 0x8000_0000;
        let mut data = ElfSection::new(".data", vec![1, 2, 3]);
        data.addr = 0x8000_1000;
        data.align = 16;
        data.flags = SHF_ALLOC | SHF_WRITE;
        let mut comment = ElfSection::new(".comment", b"fixture".to_vec());
        comment.flags = 0;
        comment.align = 1;
        let sections = vec![code.clone(), data, comment];

        for &is64 in &[false, true] {
            let bytes = build_elf(&sections, is64, code.addr).unwrap();
            assert!(!bytes.is_empty());
        }

        // Invalid mutations
        let mut bad_align = code.clone();
        bad_align.align = 3;
        assert!(build_elf(&[bad_align], true, 0).is_err());

        let mut bad_addr = code.clone();
        bad_addr.addr = u64::MAX;
        assert!(build_elf(&[bad_addr], true, 0).is_err());

        let mut bad_name = code.clone();
        bad_name.name = String::new();
        assert!(build_elf(&[bad_name], true, 0).is_err());

        let mut shstrtab_name = code.clone();
        shstrtab_name.name = ".shstrtab".to_string();
        assert!(build_elf(&[shstrtab_name], true, 0).is_err());

        let dup_name = code.clone();
        assert!(build_elf(&[dup_name.clone(), dup_name], true, 0).is_err());

        // Invalid symbol checks
        assert!(build_elf_with_symbols(
            &[code.clone()],
            &[ElfSymbol {
                name: String::new(),
                section: 0,
                offset: 0,
                size: 1,
                symbol_type: 1,
            }],
            true,
            0,
        ).is_err());

        assert!(build_elf_with_symbols(
            &[code.clone()],
            &[ElfSymbol {
                name: "x".to_string(),
                section: 1,
                offset: 0,
                size: 1,
                symbol_type: 1,
            }],
            true,
            0,
        ).is_err());

        assert!(build_elf_with_symbols(
            &[code.clone()],
            &[ElfSymbol {
                name: "x".to_string(),
                section: 0,
                offset: 4,
                size: 1,
                symbol_type: 1,
            }],
            true,
            0,
        ).is_err());
    }

    #[test]
    fn test_write_elf_output() {
        let temp_dir = std::env::temp_dir();
        let elf_path = temp_dir.join("test_write_elf.elf");
        let code = vec![0x13, 0x00, 0x00, 0x00]; // nop
        write_elf(&code, &elf_path).unwrap();

        let data = std::fs::read(&elf_path).unwrap();
        assert!(data.starts_with(b"\x7fELF\x02\x01\x01")); // ELF64 LSB
        // Verify size is compact (no 4KB padding)
        assert!(data.len() < 1000);
        let _ = std::fs::remove_file(elf_path);
    }
}
