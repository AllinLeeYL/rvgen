// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

//! Port of `riscv/zicsr.py`; function names and argument order are retained.

use super::rvprotoinstrs::*;

pub const ZICSR_OPCODE_CSR: i32 = 115;

pub fn zicsr_csrrw(rd: i32, rs1: i32, csr: i32) -> u32 {
    instruc_itype(ZICSR_OPCODE_CSR, rd, 1, rs1, csr)
}

pub fn zicsr_csrrs(rd: i32, rs1: i32, csr: i32) -> u32 {
    instruc_itype(ZICSR_OPCODE_CSR, rd, 2, rs1, csr)
}

pub fn zicsr_csrrc(rd: i32, rs1: i32, csr: i32) -> u32 {
    instruc_itype(ZICSR_OPCODE_CSR, rd, 3, rs1, csr)
}

pub fn zicsr_csrrwi(rd: i32, uimm: i32, csr: i32) -> u32 {
    instruc_itype(ZICSR_OPCODE_CSR, rd, 5, uimm, csr)
}

pub fn zicsr_csrrsi(rd: i32, uimm: i32, csr: i32) -> u32 {
    instruc_itype(ZICSR_OPCODE_CSR, rd, 6, uimm, csr)
}

pub fn zicsr_csrrci(rd: i32, uimm: i32, csr: i32) -> u32 {
    instruc_itype(ZICSR_OPCODE_CSR, rd, 7, uimm, csr)
}
