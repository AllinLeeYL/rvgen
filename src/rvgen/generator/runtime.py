"""Bare-metal entry, trap handling, and Spike HTIF termination."""

import struct

from rvgen.riscv import (CSR, rv32i_auipc, rv32i_addi, rv32i_sw, rv64i_sd,
                        rv32i_fence, rv32i_jal, zicsr_csrrw)
from rvgen.utils.elfbuilder import ElfBuilder, ElfSection, ElfSymbol, SHF_ALLOC, SHF_WRITE


def _pc_relative(delta: int) -> tuple[int, int]:
    hi = (delta + 0x800) >> 12
    if not -0x80000 <= hi <= 0x7ffff:
        raise ValueError("runtime target exceeds AUIPC range")
    return rv32i_auipc(5, hi), rv32i_addi(5, 5, delta - (hi << 12))


def executable(body: bytes, is_64bit: bool = True, start_addr: int = 0x80000000) -> bytes:
    if start_addr < 0 or start_addr % 4 or len(body) % 2:
        raise ValueError("entry must be four-byte aligned and body must contain whole instructions")
    text = bytearray(8)

    def append(*words):
        text.extend(struct.pack('<' + 'I' * len(words), *words))

    append(zicsr_csrrw(0, 5, CSR.MTVEC))
    text.extend(body)
    if len(text) % 4:
        text.extend(b'\x01\x00')  # c.nop after a compressed body
    append(rv32i_addi(31, 0, 1))  # HTIF pass
    writer_offset = len(text)
    text.extend(bytes(8))
    if not is_64bit:
        append(rv32i_sw(5, 0, 4))  # upper mailbox word
    append(rv32i_fence(0x33), rv64i_sd(5, 31, 0) if is_64bit else rv32i_sw(5, 31, 0),
           rv32i_fence(0x33), rv32i_jal(0, 0))
    trap_offset = len(text)
    append(rv32i_addi(31, 0, 3), rv32i_jal(0, writer_offset - (trap_offset + 4)))
    tohost_addr = (start_addr + len(text) + 63) & ~63
    fromhost_addr = tohost_addr + 64
    if fromhost_addr + 8 > 1 << (64 if is_64bit else 32):
        raise ValueError("runtime address overflow")
    struct.pack_into('<II', text, 0, *_pc_relative(trap_offset))
    struct.pack_into('<II', text, writer_offset, *_pc_relative(tohost_addr - start_addr - writer_offset))
    sections = [
        ElfSection('.text', bytes(text), addr=start_addr),
        ElfSection('.tohost', bytes(8), addr=tohost_addr, flags=SHF_ALLOC | SHF_WRITE, align=64),
        ElfSection('.fromhost', bytes(8), addr=fromhost_addr, flags=SHF_ALLOC | SHF_WRITE, align=64),
    ]
    symbols = [ElfSymbol('_start', 0, size=len(text), symbol_type=2),
               ElfSymbol('rvgen_trap', 0, offset=trap_offset, size=8, symbol_type=2),
               ElfSymbol('tohost', 1, size=8), ElfSymbol('fromhost', 2, size=8)]
    return ElfBuilder().build_with_symbols(sections, symbols, is_64bit, start_addr)
