// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

//! Port of `riscv/rv32i.py`; function names and argument order are retained.

use super::rvprotoinstrs::*;

pub const RV32I_OPCODE_LUI: i32 = 55;
pub const RV32I_OPCODE_AUIPC: i32 = 23;
pub const RV32I_OPCODE_JAL: i32 = 111;
pub const RV32I_OPCODE_JALR: i32 = 103;
pub const RV32I_OPCODE_B: i32 = 99;
pub const RV32I_OPCODE_L: i32 = 3;
pub const RV32I_OPCODE_S: i32 = 35;
pub const RV32I_OPCODE_ALU_IMM: i32 = 19;
pub const RV32I_OPCODE_ALU_REG: i32 = 51;
pub const RV32I_OPCODE_FEN: i32 = 15;
pub const RV32I_OPCODE_E: i32 = 115;

pub fn rv32i_lui(rd: i32, imm: i32) -> u32 {
    instruc_utype(RV32I_OPCODE_LUI, rd, imm)
}

pub fn rv32i_auipc(rd: i32, imm: i32) -> u32 {
    instruc_utype(RV32I_OPCODE_AUIPC, rd, imm)
}

pub fn rv32i_jal(rd: i32, imm: i32) -> u32 {
    instruc_jtype(RV32I_OPCODE_JAL, rd, imm)
}

pub fn rv32i_jalr(rd: i32, rs1: i32, imm: i32) -> u32 {
    instruc_itype(RV32I_OPCODE_JALR, rd, 0, rs1, imm)
}

pub fn rv32i_beq(rs1: i32, rs2: i32, imm: i32) -> u32 {
    instruc_btype(RV32I_OPCODE_B, 0, rs1, rs2, imm)
}

pub fn rv32i_bne(rs1: i32, rs2: i32, imm: i32) -> u32 {
    instruc_btype(RV32I_OPCODE_B, 1, rs1, rs2, imm)
}

pub fn rv32i_blt(rs1: i32, rs2: i32, imm: i32) -> u32 {
    instruc_btype(RV32I_OPCODE_B, 4, rs1, rs2, imm)
}

pub fn rv32i_bge(rs1: i32, rs2: i32, imm: i32) -> u32 {
    instruc_btype(RV32I_OPCODE_B, 5, rs1, rs2, imm)
}

pub fn rv32i_bltu(rs1: i32, rs2: i32, imm: i32) -> u32 {
    instruc_btype(RV32I_OPCODE_B, 6, rs1, rs2, imm)
}

pub fn rv32i_bgeu(rs1: i32, rs2: i32, imm: i32) -> u32 {
    instruc_btype(RV32I_OPCODE_B, 7, rs1, rs2, imm)
}

pub fn rv32i_lb(rd: i32, rs1: i32, imm: i32) -> u32 {
    instruc_itype(RV32I_OPCODE_L, rd, 0, rs1, imm)
}

pub fn rv32i_lh(rd: i32, rs1: i32, imm: i32) -> u32 {
    instruc_itype(RV32I_OPCODE_L, rd, 1, rs1, imm)
}

pub fn rv32i_lw(rd: i32, rs1: i32, imm: i32) -> u32 {
    instruc_itype(RV32I_OPCODE_L, rd, 2, rs1, imm)
}

pub fn rv32i_lbu(rd: i32, rs1: i32, imm: i32) -> u32 {
    instruc_itype(RV32I_OPCODE_L, rd, 4, rs1, imm)
}

pub fn rv32i_lhu(rd: i32, rs1: i32, imm: i32) -> u32 {
    instruc_itype(RV32I_OPCODE_L, rd, 5, rs1, imm)
}

pub fn rv32i_sb(rs1: i32, rs2: i32, imm: i32) -> u32 {
    instruc_stype(RV32I_OPCODE_S, 0, rs1, rs2, imm)
}

pub fn rv32i_sh(rs1: i32, rs2: i32, imm: i32) -> u32 {
    instruc_stype(RV32I_OPCODE_S, 1, rs1, rs2, imm)
}

pub fn rv32i_sw(rs1: i32, rs2: i32, imm: i32) -> u32 {
    instruc_stype(RV32I_OPCODE_S, 2, rs1, rs2, imm)
}

pub fn rv32i_addi(rd: i32, rs1: i32, imm: i32) -> u32 {
    instruc_itype(RV32I_OPCODE_ALU_IMM, rd, 0, rs1, imm)
}

pub fn rv32i_slti(rd: i32, rs1: i32, imm: i32) -> u32 {
    instruc_itype(RV32I_OPCODE_ALU_IMM, rd, 2, rs1, imm)
}

pub fn rv32i_sltiu(rd: i32, rs1: i32, imm: i32) -> u32 {
    instruc_itype(RV32I_OPCODE_ALU_IMM, rd, 3, rs1, imm)
}

pub fn rv32i_xori(rd: i32, rs1: i32, imm: i32) -> u32 {
    instruc_itype(RV32I_OPCODE_ALU_IMM, rd, 4, rs1, imm)
}

pub fn rv32i_ori(rd: i32, rs1: i32, imm: i32) -> u32 {
    instruc_itype(RV32I_OPCODE_ALU_IMM, rd, 6, rs1, imm)
}

pub fn rv32i_andi(rd: i32, rs1: i32, imm: i32) -> u32 {
    instruc_itype(RV32I_OPCODE_ALU_IMM, rd, 7, rs1, imm)
}

pub fn rv32i_slli(rd: i32, rs1: i32, shamt: i32) -> u32 {
    let imm = shamt;
    instruc_itype(RV32I_OPCODE_ALU_IMM, rd, 1, rs1, imm)
}

pub fn rv32i_srli(rd: i32, rs1: i32, shamt: i32) -> u32 {
    let imm = shamt;
    instruc_itype(RV32I_OPCODE_ALU_IMM, rd, 5, rs1, imm)
}

pub fn rv32i_srai(rd: i32, rs1: i32, shamt: i32) -> u32 {
    let imm = 1024 | shamt;
    instruc_itype(RV32I_OPCODE_ALU_IMM, rd, 5, rs1, imm)
}

pub fn rv32i_add(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32I_OPCODE_ALU_REG, rd, 0, rs1, rs2, 0)
}

pub fn rv32i_sub(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32I_OPCODE_ALU_REG, rd, 0, rs1, rs2, 32)
}

pub fn rv32i_sll(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32I_OPCODE_ALU_REG, rd, 1, rs1, rs2, 0)
}

pub fn rv32i_slt(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32I_OPCODE_ALU_REG, rd, 2, rs1, rs2, 0)
}

pub fn rv32i_sltu(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32I_OPCODE_ALU_REG, rd, 3, rs1, rs2, 0)
}

pub fn rv32i_xor(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32I_OPCODE_ALU_REG, rd, 4, rs1, rs2, 0)
}

pub fn rv32i_srl(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32I_OPCODE_ALU_REG, rd, 5, rs1, rs2, 0)
}

pub fn rv32i_sra(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32I_OPCODE_ALU_REG, rd, 5, rs1, rs2, 32)
}

pub fn rv32i_or(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32I_OPCODE_ALU_REG, rd, 6, rs1, rs2, 0)
}

pub fn rv32i_and(rd: i32, rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32I_OPCODE_ALU_REG, rd, 7, rs1, rs2, 0)
}

pub fn rv32i_fence(imm: i32) -> u32 {
    instruc_itype(RV32I_OPCODE_FEN, 0, 0, 0, imm)
}

pub fn rv32i_ecall() -> u32 {
    instruc_itype(RV32I_OPCODE_E, 0, 0, 0, 0)
}

pub fn rv32i_ebreak() -> u32 {
    instruc_itype(RV32I_OPCODE_E, 0, 0, 0, 1)
}
