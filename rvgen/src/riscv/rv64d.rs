// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

//! Port of `riscv/rv64d.py`; function names and argument order are retained.

use super::rvprotoinstrs::*;

pub const RV64D_OPCODE_FCVT: i32 = 83;

pub fn rv64d_fcvtld(rd: i32, rs1: i32, rm: i32) -> u32 {
    instruc_rtype(RV64D_OPCODE_FCVT, rd, rm, rs1, 2, 97)
}

pub fn rv64d_fcvtlud(rd: i32, rs1: i32, rm: i32) -> u32 {
    instruc_rtype(RV64D_OPCODE_FCVT, rd, rm, rs1, 3, 97)
}

pub fn rv64d_fmvxd(rd: i32, rs1: i32) -> u32 {
    instruc_rtype(RV64D_OPCODE_FCVT, rd, 0, rs1, 0, 113)
}

pub fn rv64d_fcvtdl(rd: i32, rs1: i32, rm: i32) -> u32 {
    instruc_rtype(RV64D_OPCODE_FCVT, rd, rm, rs1, 2, 105)
}

pub fn rv64d_fcvtdlu(rd: i32, rs1: i32, rm: i32) -> u32 {
    instruc_rtype(RV64D_OPCODE_FCVT, rd, rm, rs1, 3, 105)
}

pub fn rv64d_fmvdx(rd: i32, rs1: i32) -> u32 {
    instruc_rtype(RV64D_OPCODE_FCVT, rd, 0, rs1, 0, 121)
}
