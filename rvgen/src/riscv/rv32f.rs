// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

//! Port of `riscv/rv32f.py`; function names and argument order are retained.

use super::rvprotoinstrs::*;

pub const RV32F_OPCODE_FLW: i32 = 7;
pub const RV32F_OPCODE_FSW: i32 = 39;
pub const RV32F_OPCODE_FMADDS: i32 = 67;
pub const RV32F_OPCODE_FMSUBS: i32 = 71;
pub const RV32F_OPCODE_FNMSUBS: i32 = 75;
pub const RV32F_OPCODE_FNMADDS: i32 = 79;
pub const RV32F_OPCODE_FALU: i32 = 83;

pub fn rv32f_flw(rd: i32, rs1: i32, imm: i32) -> u32 {
    instruc_itype(RV32F_OPCODE_FLW, rd, 2, rs1, imm)
}

pub fn rv32f_fsw(rs1: i32, rs2: i32, imm: i32) -> u32 {
    instruc_stype(RV32F_OPCODE_FSW, 2, rs1, rs2, imm)
}

pub fn rv32f_fmadds(rd: i32, rs1: i32, rs2: i32, rs3: i32, rm: i32) -> u32 {
    instruc_r4type(RV32F_OPCODE_FMADDS, rd, rm, rs1, rs2, rs3, 0)
}

pub fn rv32f_fmsubs(rd: i32, rs1: i32, rs2: i32, rs3: i32, rm: i32) -> u32 {
    instruc_r4type(RV32F_OPCODE_FMSUBS, rd, rm, rs1, rs2, rs3, 0)
}

pub fn rv32f_fnmsubs(rd: i32, rs1: i32, rs2: i32, rs3: i32, rm: i32) -> u32 {
    instruc_r4type(RV32F_OPCODE_FNMSUBS, rd, rm, rs1, rs2, rs3, 0)
}

pub fn rv32f_fnmadds(rd: i32, rs1: i32, rs2: i32, rs3: i32, rm: i32) -> u32 {
    instruc_r4type(RV32F_OPCODE_FNMADDS, rd, rm, rs1, rs2, rs3, 0)
}

pub fn rv32f_fadds(rd: i32, rs1: i32, rs2: i32, rm: i32) -> u32 {
    instruc_rtype(RV32F_OPCODE_FALU, rd, rm, rs1, rs2, 0)
}

pub fn rv32f_fsubs(rd: i32, rs1: i32, rs2: i32, rm: i32) -> u32 {
    instruc_rtype(RV32F_OPCODE_FALU, rd, rm, rs1, rs2, 4)
}

pub fn rv32f_fmuls(rd: i32, rs1: i32, rs2: i32, rm: i32) -> u32 {
    instruc_rtype(RV32F_OPCODE_FALU, rd, rm, rs1, rs2, 8)
}

pub fn rv32f_fdivs(rd: i32, rs1: i32, rs2: i32, rm: i32) -> u32 {
    instruc_rtype(RV32F_OPCODE_FALU, rd, rm, rs1, rs2, 12)
}

pub fn rv32f_fsqrts(rd: i32, rs1: i32, rm: i32) -> u32 {
    instruc_rtype(RV32F_OPCODE_FALU, rd, rm, rs1, 0, 44)
}

pub fn rv32f_fsgnjs(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32F_OPCODE_FALU, rd, 0, rs1, rs2, 16)
}

pub fn rv32f_fsgnjns(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32F_OPCODE_FALU, rd, 1, rs1, rs2, 16)
}

pub fn rv32f_fsgnjxs(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32F_OPCODE_FALU, rd, 2, rs1, rs2, 16)
}

pub fn rv32f_fmins(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32F_OPCODE_FALU, rd, 0, rs1, rs2, 20)
}

pub fn rv32f_fmaxs(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32F_OPCODE_FALU, rd, 1, rs1, rs2, 20)
}

pub fn rv32f_fcvtws(rd: i32, rs1: i32, rm: i32) -> u32 {
    instruc_rtype(RV32F_OPCODE_FALU, rd, rm, rs1, 0, 96)
}

pub fn rv32f_fcvtwus(rd: i32, rs1: i32, rm: i32) -> u32 {
    instruc_rtype(RV32F_OPCODE_FALU, rd, rm, rs1, 1, 96)
}

pub fn rv32f_fmvxw(rd: i32, rs1: i32) -> u32 {
    instruc_rtype(RV32F_OPCODE_FALU, rd, 0, rs1, 0, 112)
}

pub fn rv32f_feqs(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32F_OPCODE_FALU, rd, 2, rs1, rs2, 80)
}

pub fn rv32f_flts(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32F_OPCODE_FALU, rd, 1, rs1, rs2, 80)
}

pub fn rv32f_fles(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32F_OPCODE_FALU, rd, 0, rs1, rs2, 80)
}

pub fn rv32f_fclasss(rd: i32, rs1: i32) -> u32 {
    instruc_rtype(RV32F_OPCODE_FALU, rd, 1, rs1, 0, 112)
}

pub fn rv32f_fcvtsw(rd: i32, rs1: i32, rm: i32) -> u32 {
    instruc_rtype(RV32F_OPCODE_FALU, rd, rm, rs1, 0, 104)
}

pub fn rv32f_fcvtswu(rd: i32, rs1: i32, rm: i32) -> u32 {
    instruc_rtype(RV32F_OPCODE_FALU, rd, rm, rs1, 1, 104)
}

pub fn rv32f_fmvwx(rd: i32, rs1: i32) -> u32 {
    instruc_rtype(RV32F_OPCODE_FALU, rd, 0, rs1, 0, 120)
}
