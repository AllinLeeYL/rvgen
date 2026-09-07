# Refer to https://en.wikipedia.org/wiki/Executable_and_Linkable_Format

from dataclasses import dataclass
from pathlib import Path
import struct

# Section flags
SHF_WRITE     = 0x1
SHF_ALLOC     = 0x2
SHF_EXECINSTR = 0x4

# Segment types & flags
PT_LOAD = 1
PF_X    = 1
PF_W    = 2
PF_R    = 4

# [Constants] Layout constants for ELF32-littleriscv
_E32_EHDR_SIZE = 52
_E32_PHDR_SIZE = 32
_E32_SHDR_SIZE = 40

# [Constants] Layout constants for ELF64-littleriscv
_E64_EHDR_SIZE = 64
_E64_PHDR_SIZE = 56
_E64_SHDR_SIZE = 64


@dataclass
class ElfSection:
    name: str
    inbytes: bytes
    addr: int = 0x0
    flags: int = SHF_ALLOC | SHF_EXECINSTR  # SHF_ALLOC | SHF_EXECINSTR
    align: int = 4
    section_type: int = 1
    link: int = 0
    info: int = 0
    entsize: int = 0


@dataclass
class ElfSymbol:
    """Global symbol defined at an offset within a zero-based section index."""
    name: str
    section: int
    offset: int = 0
    size: int = 0
    symbol_type: int = 1
    

class ElfBuilder:
    def __init__(self) -> None:
        pass

    def build_with_symbols(self, sections: list[ElfSection], symbols: list[ElfSymbol],
                           is_64bit: bool = True, start_addr: int = 0x80000000) -> bytes:
        sections = list(sections)
        if any(section.name in {".strtab", ".symtab"} for section in sections):
            raise ValueError("symbol table section names are reserved")
        strings = bytearray(b"\0")
        entry_size = 24 if is_64bit else 16
        table = bytearray(entry_size)
        names = set()
        for symbol in symbols:
            if (not symbol.name or not symbol.name.isascii() or "\0" in symbol.name
                    or symbol.name in names or not 0 <= symbol.symbol_type <= 15):
                raise ValueError("invalid or duplicate ELF symbol")
            names.add(symbol.name)
            if not 0 <= symbol.section < len(sections):
                raise ValueError("invalid symbol section")
            section = sections[symbol.section]
            if min(symbol.offset, symbol.size) < 0 or symbol.offset + symbol.size > len(section.inbytes):
                raise ValueError("symbol lies outside its section")
            name_offset = len(strings)
            strings.extend(symbol.name.encode("ascii") + b"\0")
            value = section.addr + symbol.offset
            if is_64bit:
                table.extend(struct.pack('<IBBHQQ', name_offset, 0x10 | symbol.symbol_type,
                                         0, symbol.section + 1, value, symbol.size))
            else:
                table.extend(struct.pack('<IIIBBH', name_offset, value, symbol.size,
                                         0x10 | symbol.symbol_type, 0, symbol.section + 1))
        string_index = len(sections) + 1
        sections.append(ElfSection('.strtab', bytes(strings), flags=0, align=1, section_type=3))
        sections.append(ElfSection('.symtab', bytes(table), flags=0, align=8 if is_64bit else 4,
                                   section_type=2, link=string_index, info=1, entsize=entry_size))
        return self.build(sections, is_64bit, start_addr)

    @staticmethod
    def _sh_flags_to_p_flags(sh_flags: int) -> int:
        p_flags = PF_R
        if sh_flags & SHF_WRITE:
            p_flags |= PF_W
        if sh_flags & SHF_EXECINSTR:
            p_flags |= PF_X
        return p_flags

    def build(self, sections: list[ElfSection], is_64bit: bool = True, start_addr: int = 0x80000000) -> bytes:
        """
        This function builds a little-endian RISCV executable ELF file.

        The layout of the ELF file is as follows:
        - ELF header
        - Program headers
        - Section data
        - .shstrtab
        - Section headers
        """
        # 1. Build .shstrtab dynamically
        shstrtab = bytearray(b"\x00")
        name_offsets = {}
        for s in sections:
            name_offsets[s.name] = len(shstrtab)
            shstrtab += s.name.encode("ascii") + b"\x00"
        name_offsets[".shstrtab"] = len(shstrtab)
        shstrtab.extend(b".shstrtab\x00")

        # Structure sizes
        ehdr_size = _E64_EHDR_SIZE if is_64bit else _E32_EHDR_SIZE
        phdr_size = _E64_PHDR_SIZE if is_64bit else _E32_PHDR_SIZE
        shdr_size = _E64_SHDR_SIZE if is_64bit else _E32_SHDR_SIZE

        phoff = ehdr_size
        num_phdrs = len([s for s in sections if s.flags & SHF_ALLOC])

        cur_offset = phoff + num_phdrs * phdr_size
        
        # 2. Compute file offsets for each section
        section_offsets = []
        for s in sections:
            if s.align > 1 and (cur_offset % s.align) != 0:
                cur_offset += s.align - (cur_offset % s.align)
            section_offsets.append(cur_offset)
            cur_offset += len(s.inbytes)

        # Place .shstrtab after all other sections
        shstrtab_offset = cur_offset
        cur_offset += len(shstrtab)

        if (cur_offset % 8) != 0:
            cur_offset += 8 - (cur_offset % 8)
        shoff = cur_offset
        num_shdrs = len(sections) + 2  # + NULL, .shstrtab

        # 3. Pack program headers
        phdrs_bytes = bytearray()
        for s, offset in zip(sections, section_offsets):
            if not (s.flags & SHF_ALLOC):
                continue
            p_flags = self._sh_flags_to_p_flags(s.flags)
            size = len(s.inbytes)
            if is_64bit:
                phdrs_bytes.extend(struct.pack(
                    '<IIQQQQQQ',
                    PT_LOAD, p_flags, offset, s.addr, s.addr, size, size, s.align
                ))
            else:
                phdrs_bytes.extend(struct.pack(
                    '<IIIIIIII',
                    PT_LOAD, offset, s.addr, s.addr, size, size, p_flags, s.align
                ))

        # 4. Pack section headers
        shdrs_bytes = bytearray()
        shdrs_bytes.extend(b"\x00" * shdr_size)

        for s, offset in zip(sections, section_offsets):
            sh_name = name_offsets[s.name]
            sh_type = s.section_type
            size = len(s.inbytes)
            if is_64bit:
                shdrs_bytes.extend(struct.pack(
                    '<IIQQQQIIQQ',
                    sh_name, sh_type, s.flags, s.addr, offset, size, s.link, s.info, s.align, s.entsize
                ))
            else:
                shdrs_bytes.extend(struct.pack(
                    '<IIIIIIIIII',
                    sh_name, sh_type, s.flags, s.addr, offset, size, s.link, s.info, s.align, s.entsize
                ))

        # .shstrtab section header (index = len(sections) + 1)
        shstrtab_idx = len(sections) + 1
        if is_64bit:
            shdrs_bytes.extend(struct.pack(
                '<IIQQQQIIQQ',
                name_offsets[".shstrtab"], 3, 0, 0, shstrtab_offset, len(shstrtab), 0, 0, 1, 0
            ))
        else:
            shdrs_bytes.extend(struct.pack(
                '<IIIIIIIIII',
                name_offsets[".shstrtab"], 3, 0, 0, shstrtab_offset, len(shstrtab), 0, 0, 1, 0
            ))

        # 5. Pack the ELF header
        if is_64bit:
            e_ident = b"\x7fELF\x02\x01\x01\x00" + b"\x00" * 8
            ehdr_bytes = e_ident + struct.pack(
                '<HHIQQQIHHHHHH',
                2, 0xf3, 1, start_addr, phoff, shoff, 0,
                ehdr_size, phdr_size, num_phdrs, shdr_size, num_shdrs, shstrtab_idx
            )
        else:
            e_ident = b"\x7fELF\x01\x01\x01\x00" + b"\x00" * 8
            ehdr_bytes = e_ident + struct.pack(
                '<HHIIIIIHHHHHH',
                2, 0xf3, 1, start_addr, phoff, shoff, 0,
                ehdr_size, phdr_size, num_phdrs, shdr_size, num_shdrs, shstrtab_idx
            )

        # 6. Concatenate everything
        file_bytes = bytearray(ehdr_bytes + phdrs_bytes)
        for s, offset in zip(sections, section_offsets):
            if len(file_bytes) < offset:
                file_bytes.extend(b"\x00" * (offset - len(file_bytes)))
            file_bytes.extend(s.inbytes)

        if len(file_bytes) < shstrtab_offset:
            file_bytes.extend(b"\x00" * (shstrtab_offset - len(file_bytes)))
        file_bytes.extend(shstrtab)

        if len(file_bytes) < shoff:
            file_bytes.extend(b"\x00" * (shoff - len(file_bytes)))
        file_bytes.extend(shdrs_bytes)
        
        return bytes(file_bytes)

    def save(self, elf_bytes: bytes, destination_path: str | Path) -> None:
        with open(destination_path, 'wb') as f:
            f.write(elf_bytes)


# def gen_elf(inbytes: bytes, start_addr: int, section_addr: int, destination_path: str, is_64bit: bool = True) -> None:
#     """
#     Generate a RISCV executable ELF file.

#     Args:
#         inbytes: The bytes to put into the ELF file.
#                  Must be in little endian format.
#         start_addr: The entry PC.
#         section_addr: The address at which to load the .text.init section.
#                       May be None to use ``start_addr``.
#         destination_path: Output ELF path.
#         is_64bit: If True emit ELF64-littleriscv, else ELF32-littleriscv.
#     """
#     if section_addr is None:
#         section_addr = start_addr

#     if is_64bit:
#         elf_bytes = _build_elf64_riscv(inbytes, start_addr, section_addr)
#     else:
#         elf_bytes = _build_elf32_riscv(inbytes, start_addr, section_addr)

#     with open(destination_path, 'wb') as f:
#         f.write(elf_bytes)
