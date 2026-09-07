// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

//! Port of `riscv/rv32d.py`; function names and argument order are retained.

use super::rvprotoinstrs::*;

pub const RV32D_OPCODE_FLD: i32 = 7;
pub const RV32D_OPCODE_FSD: i32 = 39;
pub const RV32D_OPCODE_FMADDD: i32 = 67;
pub const RV32D_OPCODE_FMSUBD: i32 = 71;
pub const RV32D_OPCODE_FNMSUBD: i32 = 75;
pub const RV32D_OPCODE_FNMADDD: i32 = 79;
pub const RV32D_OPCODE_FALU: i32 = 83;

pub fn rv32d_fld(rd: i32, rs1: i32, imm: i32) -> u32 {
    instruc_itype(RV32D_OPCODE_FLD, rd, 3, rs1, imm)
}

pub fn rv32d_fsd(rs1: i32, rs2: i32, imm: i32) -> u32 {
    instruc_stype(RV32D_OPCODE_FSD, 3, rs1, rs2, imm)
}

pub fn rv32d_fmaddd(rd: i32, rs1: i32, rs2: i32, rs3: i32, rm: i32) -> u32 {
    instruc_r4type(RV32D_OPCODE_FMADDD, rd, rm, rs1, rs2, rs3, 1)
}

pub fn rv32d_fmsubd(rd: i32, rs1: i32, rs2: i32, rs3: i32, rm: i32) -> u32 {
    instruc_r4type(RV32D_OPCODE_FMSUBD, rd, rm, rs1, rs2, rs3, 1)
}

pub fn rv32d_fnmsubd(rd: i32, rs1: i32, rs2: i32, rs3: i32, rm: i32) -> u32 {
    instruc_r4type(RV32D_OPCODE_FNMSUBD, rd, rm, rs1, rs2, rs3, 1)
}

pub fn rv32d_fnmaddd(rd: i32, rs1: i32, rs2: i32, rs3: i32, rm: i32) -> u32 {
    instruc_r4type(RV32D_OPCODE_FNMADDD, rd, rm, rs1, rs2, rs3, 1)
}

pub fn rv32d_faddd(rd: i32, rs1: i32, rs2: i32, rm: i32) -> u32 {
    instruc_rtype(RV32D_OPCODE_FALU, rd, rm, rs1, rs2, 1)
}

pub fn rv32d_fsubd(rd: i32, rs1: i32, rs2: i32, rm: i32) -> u32 {
    instruc_rtype(RV32D_OPCODE_FALU, rd, rm, rs1, rs2, 5)
}

pub fn rv32d_fmuld(rd: i32, rs1: i32, rs2: i32, rm: i32) -> u32 {
    instruc_rtype(RV32D_OPCODE_FALU, rd, rm, rs1, rs2, 9)
}

pub fn rv32d_fdivd(rd: i32, rs1: i32, rs2: i32, rm: i32) -> u32 {
    instruc_rtype(RV32D_OPCODE_FALU, rd, rm, rs1, rs2, 13)
}

pub fn rv32d_fsqrtd(rd: i32, rs1: i32, rm: i32) -> u32 {
    instruc_rtype(RV32D_OPCODE_FALU, rd, rm, rs1, 0, 45)
}

pub fn rv32d_fsgnjd(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32D_OPCODE_FALU, rd, 0, rs1, rs2, 17)
}

pub fn rv32d_fsgnjnd(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32D_OPCODE_FALU, rd, 1, rs1, rs2, 17)
}

pub fn rv32d_fsgnjxd(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32D_OPCODE_FALU, rd, 2, rs1, rs2, 17)
}

pub fn rv32d_fmind(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32D_OPCODE_FALU, rd, 0, rs1, rs2, 21)
}

pub fn rv32d_fmaxd(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32D_OPCODE_FALU, rd, 1, rs1, rs2, 21)
}

pub fn rv32d_fcvtsd(rd: i32, rs1: i32, rm: i32) -> u32 {
    instruc_rtype(RV32D_OPCODE_FALU, rd, rm, rs1, 1, 32)
}

pub fn rv32d_fcvtds(rd: i32, rs1: i32, rm: i32) -> u32 {
    instruc_rtype(RV32D_OPCODE_FALU, rd, rm, rs1, 0, 33)
}

pub fn rv32d_feqd(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32D_OPCODE_FALU, rd, 2, rs1, rs2, 81)
}

pub fn rv32d_fltd(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32D_OPCODE_FALU, rd, 1, rs1, rs2, 81)
}

pub fn rv32d_fled(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32D_OPCODE_FALU, rd, 0, rs1, rs2, 81)
}

pub fn rv32d_fclassd(rd: i32, rs1: i32) -> u32 {
    instruc_rtype(RV32D_OPCODE_FALU, rd, 1, rs1, 0, 113)
}

pub fn rv32d_fcvtwd(rd: i32, rs1: i32, rm: i32) -> u32 {
    instruc_rtype(RV32D_OPCODE_FALU, rd, rm, rs1, 0, 97)
}

pub fn rv32d_fcvtwud(rd: i32, rs1: i32, rm: i32) -> u32 {
    instruc_rtype(RV32D_OPCODE_FALU, rd, rm, rs1, 1, 97)
}

pub fn rv32d_fcvtdw(rd: i32, rs1: i32, rm: i32) -> u32 {
    instruc_rtype(RV32D_OPCODE_FALU, rd, rm, rs1, 0, 105)
}

pub fn rv32d_fcvtdwu(rd: i32, rs1: i32, rm: i32) -> u32 {
    instruc_rtype(RV32D_OPCODE_FALU, rd, rm, rs1, 1, 105)
}
