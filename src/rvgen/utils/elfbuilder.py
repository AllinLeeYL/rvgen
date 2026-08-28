# Refer to https://en.wikipedia.org/wiki/Executable_and_Linkable_Format

from dataclasses import dataclass
from pathlib import Path
from typing import Any
import struct

# [Constants] Layout constants for ELF32-littleriscv
_E32_EHDR_SIZE = 52
_E32_PHDR_SIZE = 32
_E32_SHDR_SIZE = 40

# [Constants] Layout constants for ELF64-littleriscv
_E64_EHDR_SIZE = 64
_E64_PHDR_SIZE = 56
_E64_SHDR_SIZE = 64

_SHSTRTAB_BYTES = b"\x00.shstrtab\x00"
_NUM_SHDRS = 4  # NULL, .shstrtab, .text


@dataclass
class ElfSection:
    name: str
    inbytes: bytes
    sec_addr: int = 0x0



def _build_elf32_riscv(inbytes: bytes, start_addr: int, section_addr: int) -> bytes:
    section_size = len(inbytes)
    phoff = _E32_EHDR_SIZE
    shoff = phoff + _E32_PHDR_SIZE
    shstrtab_offset = shoff + _NUM_SHDRS * _E32_SHDR_SIZE
    section_data_offset = shstrtab_offset + len(_SHSTRTAB_BYTES)

    e_ident = b"\x7fELF\x01\x01\x01\x00" + b"\x00" * 8
    ehdr = e_ident + struct.pack(
        '<HHIIIIIHHHHHH',
        2, 0xf3, 1, start_addr,
        phoff, shoff, 0,
        _E32_EHDR_SIZE, _E32_PHDR_SIZE, 1, _E32_SHDR_SIZE, _NUM_SHDRS, 1,
    )
    phdr = struct.pack(
        '<IIIIIIII',
        1,                       # PT_LOAD
        section_data_offset,
        section_addr, section_addr,
        section_size, section_size,
        7,                       # PF_R | PF_W | PF_X
        1,
    )
    shdr_null = b"\x00" * _E32_SHDR_SIZE
    shdr_shstrtab = struct.pack(
        '<IIIIIIIIII',
        1, 3, 0, 0,
        shstrtab_offset, len(_SHSTRTAB_BYTES),
        0, 0, 1, 0,
    )
    shdr_text = struct.pack(
        '<IIIIIIIIII',
        11, 1, 6, section_addr,
        section_data_offset, section_size,
        0, 0, 4, 0,
    )
    return ehdr + phdr + shdr_null + shdr_shstrtab + shdr_text + _SHSTRTAB_BYTES + inbytes


def _build_elf64_riscv(sections: list[ElfSection], start_addr: int) -> bytes:
    section_size = len(inbytes)
    num_shdrs = len(sections) + 2
    shstrtab_bytes = _SHSTRTAB_BYTES.copy()
    for section in sections:
        shstrtab_bytes += section.name.encode() + b"\x00"
    

    phoff = _E64_EHDR_SIZE
    shoff = phoff + _E64_PHDR_SIZE
    shstrtab_offset = shoff + num_shdrs * _E64_SHDR_SIZE
    section_data_offset = shstrtab_offset + len(shstrtab_bytes)

    # [ELF header] ELFCLASS64, little-endian
    e_ident = b"\x7fELF\x02\x01\x01\x00" + b"\x00" * 8
    ehdr = e_ident + struct.pack(
        '<HHIQQQIHHHHHH',
        2,            # e_type ET_EXEC
        0xf3,         # e_machine EM_RISCV
        1,            # e_version
        start_addr,   # e_entry (64-bit)
        phoff,        # e_phoff (64-bit)
        shoff,        # e_shoff (64-bit)
        0,            # e_flags
        _E64_EHDR_SIZE,
        _E64_PHDR_SIZE, 1,
        _E64_SHDR_SIZE, _NUM_SHDRS, 1,
    )
    # [Program Header] Phdr64: p_type, p_flags, p_offset, p_vaddr, p_paddr, p_filesz, p_memsz, p_align
    phdr = struct.pack(
        '<IIQQQQQQ',
        1,                       # p_type PT_LOAD
        7,                       # p_flags
        section_data_offset,
        section_addr, section_addr,
        section_size, section_size,
        1,                       # p_align
    )
    # [Section Header]
    shdr_null = b"\x00" * _E64_SHDR_SIZE
    shdr_shstrtab = struct.pack(
        '<IIQQQQIIQQ',
        1, 3, 0, 0,
        shstrtab_offset, len(_SHSTRTAB_BYTES),
        0, 0, 1, 0,
    )
    shdr_text = struct.pack(
        '<IIQQQQIIQQ',
        11, 1, 6, section_addr,
        section_data_offset, section_size,
        0, 0, 4, 0,
    )
    return ehdr + phdr + shdr_null + shdr_shstrtab + shdr_text + _SHSTRTAB_BYTES + inbytes
    

class ElfBuilder:
    def __init__(self) -> None:
        pass

    def build(self, sections: list[ElfSection], is_64bit: bool = True, start_addr: int = 0x80000000) -> bytes:
        if is_64bit:
            elf_bytes = _build_elf64_riscv(sections, start_addr)
        else:
            elf_bytes = _build_elf32_riscv(sections, start_addr)
        return elf_bytes

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
