// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

//! Port of `riscv/rv64f.py`; function names and argument order are retained.

use super::rvprotoinstrs::*;

pub const RV64F_OPCODE_FCVT: i32 = 83;

pub fn rv64f_fcvtls(rd: i32, rs1: i32, rm: i32) -> u32 {
    instruc_rtype(RV64F_OPCODE_FCVT, rd, rm, rs1, 2, 96)
}

pub fn rv64f_fcvtlus(rd: i32, rs1: i32, rm: i32) -> u32 {
    instruc_rtype(RV64F_OPCODE_FCVT, rd, rm, rs1, 3, 96)
}

pub fn rv64f_fcvtsl(rd: i32, rs1: i32, rm: i32) -> u32 {
    instruc_rtype(RV64F_OPCODE_FCVT, rd, rm, rs1, 2, 104)
}

pub fn rv64f_fcvtslu(rd: i32, rs1: i32, rm: i32) -> u32 {
    instruc_rtype(RV64F_OPCODE_FCVT, rd, rm, rs1, 3, 104)
}
