//! Little-endian RISC-V ELF32/ELF64 executable writer.
//! Layout and header defaults match the original Python ElfBuilder.

use std::{collections::HashSet, path::Path};

use crate::Result;

pub const SHF_WRITE: u64 = 1;
pub const SHF_ALLOC: u64 = 2;
pub const SHF_EXECINSTR: u64 = 4;
pub const PT_LOAD: u32 = 1;
pub const PF_X: u32 = 1;
pub const PF_W: u32 = 2;
pub const PF_R: u32 = 4;

#[derive(Debug, Clone)]
pub struct ElfSection {
    pub name: String,
    pub inbytes: Vec<u8>,
    pub addr: u64,
    pub flags: u64,
    pub align: u64,
    pub section_type: u32,
    pub link: u32,
    pub info: u32,
    pub entsize: u64,
}

impl ElfSection {
    pub fn new(name: impl Into<String>, inbytes: impl Into<Vec<u8>>) -> Self {
        Self {
            name: name.into(),
            inbytes: inbytes.into(),
            addr: 0,
            flags: SHF_ALLOC | SHF_EXECINSTR,
            align: 4,
            section_type: 1,
            link: 0,
            info: 0,
            entsize: 0,
        }
    }
}

/// A global symbol defined at an offset within a section (zero-based index).
#[derive(Debug, Clone)]
pub struct ElfSymbol {
    pub name: String,
    pub section: usize,
    pub offset: u64,
    pub size: u64,
    /// ELF STT_OBJECT = 1, STT_FUNC = 2.
    pub symbol_type: u8,
}

#[derive(Debug, Default)]
pub struct ElfBuilder;

fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b)
        .ok_or_else(|| "ELF layout exceeds 64 bits".into())
}

fn align_up(offset: u64, alignment: u64) -> Result<u64> {
    let alignment = alignment.max(1);
    add(offset, (alignment - offset % alignment) % alignment)
}

fn w16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn w32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn w64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn word(bytes: &mut Vec<u8>, value: u64, is_64bit: bool) -> Result<()> {
    if is_64bit {
        w64(bytes, value);
    } else {
        w32(
            bytes,
            value.try_into().map_err(|_| "value does not fit ELF32")?,
        );
    }
    Ok(())
}

impl ElfBuilder {
    pub fn build_with_symbols(
        &self,
        sections: &[ElfSection],
        symbols: &[ElfSymbol],
        is_64bit: bool,
        start_addr: u64,
    ) -> Result<Vec<u8>> {
        let mut sections = sections.to_vec();
        let mut names = HashSet::new();
        let mut strings = vec![0];
        let entry_size = if is_64bit { 24 } else { 16 };
        let mut table = vec![0; entry_size]; // STN_UNDEF, the only local symbol
        for symbol in symbols {
            if symbol.name.is_empty()
                || !symbol.name.is_ascii()
                || symbol.name.contains('\0')
                || !names.insert(&symbol.name)
                || symbol.symbol_type > 15
            {
                return Err("invalid or duplicate ELF symbol".into());
            }
            let section = sections
                .get(symbol.section)
                .ok_or("invalid symbol section")?;
            if add(symbol.offset, symbol.size)? > section.inbytes.len() as u64 {
                return Err("symbol lies outside its section".into());
            }
            let value = add(section.addr, symbol.offset)?;
            let index = u16::try_from(symbol.section + 1)?;
            w32(&mut table, u32::try_from(strings.len())?);
            strings.extend_from_slice(symbol.name.as_bytes());
            strings.push(0);
            if is_64bit {
                table.extend_from_slice(&[0x10 | symbol.symbol_type, 0]);
                w16(&mut table, index);
                w64(&mut table, value);
                w64(&mut table, symbol.size);
            } else {
                word(&mut table, value, false)?;
                word(&mut table, symbol.size, false)?;
                table.extend_from_slice(&[0x10 | symbol.symbol_type, 0]);
                w16(&mut table, index);
            }
        }
        let string_index = u32::try_from(sections.len() + 1)?;
        let mut strtab = ElfSection::new(".strtab", strings);
        strtab.flags = 0;
        strtab.align = 1;
        strtab.section_type = 3;
        sections.push(strtab);
        let mut symtab = ElfSection::new(".symtab", table);
        symtab.flags = 0;
        symtab.align = if is_64bit { 8 } else { 4 };
        symtab.section_type = 2;
        symtab.link = string_index;
        symtab.info = 1;
        symtab.entsize = entry_size as u64;
        sections.push(symtab);
        self.build(&sections, is_64bit, start_addr)
    }

    pub fn sh_flags_to_p_flags(flags: u64) -> u32 {
        PF_R | if flags & SHF_WRITE != 0 { PF_W } else { 0 }
            | if flags & SHF_EXECINSTR != 0 { PF_X } else { 0 }
    }

    pub fn build(
        &self,
        sections: &[ElfSection],
        is_64bit: bool,
        start_addr: u64,
    ) -> Result<Vec<u8>> {
        // Extended section numbering is intentionally outside this small writer's scope.
        if sections.len() + 2 >= 0xff00 {
            return Err("too many ELF sections".into());
        }
        let (ehsize, phsize, shsize) = if is_64bit { (64, 56, 64) } else { (52, 32, 40) };
        let phnum = sections.iter().filter(|s| s.flags & SHF_ALLOC != 0).count();
        let mut names = HashSet::new();
        let mut shstrtab = vec![0];
        let mut name_offsets = Vec::new();
        let mut section_offsets = Vec::new();
        let mut offset = ehsize + phnum as u64 * phsize;
        for section in sections {
            if section.name.is_empty()
                || !section.name.is_ascii()
                || section.name.contains('\0')
                || section.name == ".shstrtab"
                || !names.insert(&section.name)
            {
                return Err(
                    format!("invalid or duplicate section name: {:?}", section.name).into(),
                );
            }
            if section.align > 1 && !section.align.is_power_of_two() {
                return Err(
                    format!("section {} alignment must be a power of two", section.name).into(),
                );
            }
            let end_addr = add(section.addr, section.inbytes.len() as u64)?;
            if !is_64bit && (section.addr > u32::MAX as u64 || end_addr > 1_u64 << 32) {
                return Err("section address does not fit ELF32".into());
            }
            name_offsets.push(u32::try_from(shstrtab.len())?);
            shstrtab.extend_from_slice(section.name.as_bytes());
            shstrtab.push(0);
            offset = align_up(offset, section.align)?;
            section_offsets.push(offset);
            offset = add(offset, section.inbytes.len() as u64)?;
        }
        let shstrtab_name = u32::try_from(shstrtab.len())?;
        shstrtab.extend_from_slice(b".shstrtab\0");
        let shstrtab_offset = offset;
        let shoff = align_up(add(offset, shstrtab.len() as u64)?, 8)?;
        let file_size = add(shoff, (sections.len() as u64 + 2) * shsize)?;
        if !is_64bit && (file_size > u32::MAX as u64 || start_addr > u32::MAX as u64) {
            return Err("ELF layout or entry address does not fit ELF32".into());
        }
        let file_size = usize::try_from(file_size)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(file_size)?;

        bytes.extend_from_slice(b"\x7fELF");
        bytes.extend_from_slice(&[if is_64bit { 2 } else { 1 }, 1, 1, 0]);
        bytes.extend_from_slice(&[0; 8]);
        w16(&mut bytes, 2); // ET_EXEC
        w16(&mut bytes, 0xf3); // EM_RISCV
        w32(&mut bytes, 1);
        word(&mut bytes, start_addr, is_64bit)?;
        word(&mut bytes, ehsize, is_64bit)?;
        word(&mut bytes, shoff, is_64bit)?;
        w32(&mut bytes, 0); // e_flags
        for value in [
            ehsize as u16,
            phsize as u16,
            phnum as u16,
            shsize as u16,
            (sections.len() + 2) as u16,
            (sections.len() + 1) as u16,
        ] {
            w16(&mut bytes, value);
        }

        for (section, &file_offset) in sections.iter().zip(&section_offsets) {
            if section.flags & SHF_ALLOC == 0 {
                continue;
            }
            let flags = Self::sh_flags_to_p_flags(section.flags);
            w32(&mut bytes, PT_LOAD);
            if is_64bit {
                w32(&mut bytes, flags);
            }
            for value in [
                file_offset,
                section.addr,
                section.addr,
                section.inbytes.len() as u64,
                section.inbytes.len() as u64,
            ] {
                word(&mut bytes, value, is_64bit)?;
            }
            if !is_64bit {
                w32(&mut bytes, flags);
            }
            word(&mut bytes, section.align, is_64bit)?;
        }
        for (section, &file_offset) in sections.iter().zip(&section_offsets) {
            bytes.resize(file_offset as usize, 0);
            bytes.extend_from_slice(&section.inbytes);
        }
        bytes.extend_from_slice(&shstrtab);
        bytes.resize((shoff + shsize) as usize, 0); // alignment and null section header

        let string_section = ElfSection {
            name: ".shstrtab".into(),
            inbytes: shstrtab,
            addr: 0,
            flags: 0,
            align: 1,
            section_type: 3,
            link: 0,
            info: 0,
            entsize: 0,
        };
        for ((section, name_offset), file_offset) in sections
            .iter()
            .chain([&string_section])
            .zip(name_offsets.into_iter().chain([shstrtab_name]))
            .zip(section_offsets.into_iter().chain([shstrtab_offset]))
        {
            w32(&mut bytes, name_offset);
            w32(&mut bytes, section.section_type);
            for value in [
                section.flags,
                section.addr,
                file_offset,
                section.inbytes.len() as u64,
            ] {
                word(&mut bytes, value, is_64bit)?;
            }
            w32(&mut bytes, section.link);
            w32(&mut bytes, section.info);
            word(&mut bytes, section.align, is_64bit)?;
            word(&mut bytes, section.entsize, is_64bit)?;
        }
        debug_assert_eq!(bytes.len(), file_size);
        Ok(bytes)
    }

    pub fn save(&self, elf_bytes: &[u8], destination_path: impl AsRef<Path>) -> Result<()> {
        let path = destination_path.as_ref();
        std::fs::write(path, elf_bytes)
            .map_err(|error| format!("cannot write {}: {error}", path.display()).into())
    }
}
