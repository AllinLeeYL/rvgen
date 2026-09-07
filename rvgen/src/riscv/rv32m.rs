// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

//! Port of `riscv/rv32m.py`; function names and argument order are retained.

use super::rvprotoinstrs::*;

pub const RV32M_OPCODE_MUL: i32 = 51;

pub fn rv32m_mul(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32M_OPCODE_MUL, rd, 0, rs1, rs2, 1)
}

pub fn rv32m_mulh(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32M_OPCODE_MUL, rd, 1, rs1, rs2, 1)
}

pub fn rv32m_mulhsu(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32M_OPCODE_MUL, rd, 2, rs1, rs2, 1)
}

pub fn rv32m_mulhu(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32M_OPCODE_MUL, rd, 3, rs1, rs2, 1)
}

pub fn rv32m_div(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32M_OPCODE_MUL, rd, 4, rs1, rs2, 1)
}

pub fn rv32m_divu(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32M_OPCODE_MUL, rd, 5, rs1, rs2, 1)
}

pub fn rv32m_rem(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32M_OPCODE_MUL, rd, 6, rs1, rs2, 1)
}

pub fn rv32m_remu(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32M_OPCODE_MUL, rd, 7, rs1, rs2, 1)
}
