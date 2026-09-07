// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

//! Port of `riscv/rv64m.py`; function names and argument order are retained.

use super::rvprotoinstrs::*;

pub const RV64M_OPCODE_MUL: i32 = 59;

pub fn rv64m_mulw(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV64M_OPCODE_MUL, rd, 0, rs1, rs2, 1)
}

pub fn rv64m_divw(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV64M_OPCODE_MUL, rd, 4, rs1, rs2, 1)
}

pub fn rv64m_divuw(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV64M_OPCODE_MUL, rd, 5, rs1, rs2, 1)
}

pub fn rv64m_remw(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV64M_OPCODE_MUL, rd, 6, rs1, rs2, 1)
}

pub fn rv64m_remuw(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV64M_OPCODE_MUL, rd, 7, rs1, rs2, 1)
}
