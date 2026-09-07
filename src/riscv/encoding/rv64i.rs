// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

//! Port of `riscv/rv64i.py`; function names and argument order are retained.

use super::rvprotoinstrs::*;

pub const RV64I_OPCODE_LWU: i32 = 3;
pub const RV64I_OPCODE_LD: i32 = 3;
pub const RV64I_OPCODE_SD: i32 = 35;
pub const RV64I_OPCODE_ALU_IMM: i32 = 27;
pub const RV64I_OPCODE_ALU_REG: i32 = 59;

pub fn rv64i_lwu(rd: i32, rs1: i32, imm: i32) -> u32 {
    instruc_itype(RV64I_OPCODE_LWU, rd, 6, rs1, imm)
}

pub fn rv64i_ld(rd: i32, rs1: i32, imm: i32) -> u32 {
    instruc_itype(RV64I_OPCODE_LD, rd, 3, rs1, imm)
}

pub fn rv64i_sd(rs1: i32, rs2: i32, imm: i32) -> u32 {
    instruc_stype(RV64I_OPCODE_SD, 3, rs1, rs2, imm)
}

pub fn rv64i_addiw(rd: i32, rs1: i32, imm: i32) -> u32 {
    instruc_itype(RV64I_OPCODE_ALU_IMM, rd, 0, rs1, imm)
}

pub fn rv64i_slliw(rd: i32, rs1: i32, shamt: i32) -> u32 {
    let imm = shamt;
    instruc_itype(RV64I_OPCODE_ALU_IMM, rd, 1, rs1, imm)
}

pub fn rv64i_srliw(rd: i32, rs1: i32, shamt: i32) -> u32 {
    let imm = shamt;
    instruc_itype(RV64I_OPCODE_ALU_IMM, rd, 5, rs1, imm)
}

pub fn rv64i_sraiw(rd: i32, rs1: i32, shamt: i32) -> u32 {
    let imm = 1024 | shamt;
    instruc_itype(RV64I_OPCODE_ALU_IMM, rd, 5, rs1, imm)
}

pub fn rv64i_addw(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV64I_OPCODE_ALU_REG, rd, 0, rs1, rs2, 0)
}

pub fn rv64i_subw(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV64I_OPCODE_ALU_REG, rd, 0, rs1, rs2, 32)
}

pub fn rv64i_sllw(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV64I_OPCODE_ALU_REG, rd, 1, rs1, rs2, 0)
}

pub fn rv64i_srlw(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV64I_OPCODE_ALU_REG, rd, 5, rs1, rs2, 0)
}

pub fn rv64i_sraw(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV64I_OPCODE_ALU_REG, rd, 5, rs1, rs2, 32)
}
