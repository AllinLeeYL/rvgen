// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

//! Port of `riscv/rv64c.py`; function names and argument order are retained.

use super::rvprotoinstrs::*;

pub const RV64IC_OPCODE_MISC_ALU: (i32, i32) = (4, 1);
pub const RV64IC_OPCODE_ADDIW: (i32, i32) = (1, 1);
pub const RV64IC_OPCODE_LDSP: (i32, i32) = (3, 2);
pub const RV64IC_OPCODE_LD: (i32, i32) = (3, 0);
pub const RV64IC_OPCODE_SDSP: (i32, i32) = (7, 2);
pub const RV64IC_OPCODE_SD: (i32, i32) = (7, 0);

pub fn rv64ic_addw(rd: i32, rs2: i32) -> u32 {
    let rdprime = rd - 8;
    let rs2prime = rs2 - 8;
    let funct6 = RV64IC_OPCODE_MISC_ALU.0 << 3 | 7;
    instruc_catype(RV64IC_OPCODE_MISC_ALU.1, rs2prime, 1, rdprime, funct6)
}

pub fn rv64ic_subw(rd: i32, rs2: i32) -> u32 {
    let rdprime = rd - 8;
    let rs2prime = rs2 - 8;
    let funct6 = RV64IC_OPCODE_MISC_ALU.0 << 3 | 7;
    instruc_catype(RV64IC_OPCODE_MISC_ALU.1, rs2prime, 0, rdprime, funct6)
}

pub fn rv64ic_addiw(rd: i32, imm: i32) -> u32 {
    instruc_citype(RV64IC_OPCODE_ADDIW.1, rd, RV64IC_OPCODE_ADDIW.0, imm)
}

pub fn rv64ic_ldsp(rd: i32, imm: i32) -> u32 {
    let bit_5 = imm >> 5 & 1;
    let bit_4_to_3 = imm >> 3 & 3;
    let bit_8_to_6 = imm >> 6 & 7;
    let imm = bit_5 << 5 | bit_4_to_3 << 3 | bit_8_to_6;
    instruc_citype(RV64IC_OPCODE_LDSP.1, rd, RV64IC_OPCODE_LDSP.0, imm)
}

pub fn rv64ic_ld(rd: i32, rs1: i32, imm: i32) -> u32 {
    let rdprime = rd - 8;
    let rs1prime = rs1 - 8;
    let bit_5_to_3 = imm >> 3 & 7;
    let bit_7_to_6 = imm >> 6 & 3;
    let imm = bit_5_to_3 << 2 | bit_7_to_6;
    instruc_cltype(
        RV64IC_OPCODE_LD.1,
        rdprime,
        rs1prime,
        RV64IC_OPCODE_LD.0,
        imm,
    )
}

pub fn rv64ic_sdsp(rs2: i32, imm: i32) -> u32 {
    let bit_5_to_3 = imm >> 2 & 7;
    let bit_8_to_6 = imm >> 6 & 7;
    let imm = bit_5_to_3 << 3 | bit_8_to_6;
    instruc_csstype(RV64IC_OPCODE_SDSP.1, rs2, RV64IC_OPCODE_SDSP.0, imm)
}

pub fn rv64ic_sd(rs1: i32, rs2: i32, imm: i32) -> u32 {
    let rs1prime = rs1 - 8;
    let rs2prime = rs2 - 8;
    let bit_5_to_3 = imm >> 3 & 7;
    let bit_7_to_6 = imm >> 6 & 3;
    let imm = bit_5_to_3 << 2 | bit_7_to_6;
    instruc_cstype(
        RV64IC_OPCODE_SD.1,
        rs1prime,
        rs2prime,
        RV64IC_OPCODE_SD.0,
        imm,
    )
}
