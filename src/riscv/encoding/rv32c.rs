// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

//! Port of `riscv/rv32c.py`; function names and argument order are retained.

use super::rvprotoinstrs::*;

pub const RV32IC_OPCODE_MV_ADD: (i32, i32) = (4, 2);
pub const RV32IC_OPCODE_MISC_ALU: (i32, i32) = (4, 1);
pub const RV32IC_OPCODE_LUI_ADDI16SP: (i32, i32) = (3, 1);
pub const RV32IC_OPCODE_ADDI4SPN: (i32, i32) = (0, 0);
pub const RV32IC_OPCODE_ADDI: (i32, i32) = (0, 1);
pub const RV32IC_OPCODE_LI: (i32, i32) = (2, 1);
pub const RV32IC_OPCODE_SLLI: (i32, i32) = (0, 2);
pub const RV32IC_OPCODE_BEQZ: (i32, i32) = (6, 1);
pub const RV32IC_OPCODE_BNEZ: (i32, i32) = (7, 1);
pub const RV32IC_OPCODE_J: (i32, i32) = (5, 1);
pub const RV32IC_OPCODE_JAL: (i32, i32) = (1, 1);
pub const RV32IC_OPCODE_JALR_JR: (i32, i32) = (4, 2);
pub const RV32IC_OPCODE_LWSP: (i32, i32) = (2, 2);
pub const RV32IC_OPCODE_LW: (i32, i32) = (2, 0);
pub const RV32IC_OPCODE_SWSP: (i32, i32) = (6, 2);
pub const RV32IC_OPCODE_SW: (i32, i32) = (6, 0);

pub fn rv32ic_mv(rd: i32, rs2: i32) -> u32 {
    let funct4 = RV32IC_OPCODE_MV_ADD.0 << 1;
    instruc_crtype(RV32IC_OPCODE_MV_ADD.1, rs2, rd, funct4)
}

pub fn rv32ic_add(rd: i32, rs2: i32) -> u32 {
    let funct4 = RV32IC_OPCODE_MV_ADD.0 << 1 | 1;
    instruc_crtype(RV32IC_OPCODE_MV_ADD.1, rs2, rd, funct4)
}

pub fn rv32ic_and(rd: i32, rs2: i32) -> u32 {
    let rdprime = rd - 8;
    let rs2prime = rs2 - 8;
    let funct6 = RV32IC_OPCODE_MISC_ALU.0 << 3 | 3;
    instruc_catype(RV32IC_OPCODE_MISC_ALU.1, rs2prime, 3, rdprime, funct6)
}

pub fn rv32ic_or(rd: i32, rs2: i32) -> u32 {
    let rdprime = rd - 8;
    let rs2prime = rs2 - 8;
    let funct6 = RV32IC_OPCODE_MISC_ALU.0 << 3 | 3;
    instruc_catype(RV32IC_OPCODE_MISC_ALU.1, rs2prime, 2, rdprime, funct6)
}

pub fn rv32ic_xor(rd: i32, rs2: i32) -> u32 {
    let rdprime = rd - 8;
    let rs2prime = rs2 - 8;
    let funct6 = RV32IC_OPCODE_MISC_ALU.0 << 3 | 3;
    instruc_catype(RV32IC_OPCODE_MISC_ALU.1, rs2prime, 1, rdprime, funct6)
}

pub fn rv32ic_sub(rd: i32, rs2: i32) -> u32 {
    let rdprime = rd - 8;
    let rs2prime = rs2 - 8;
    let funct6 = RV32IC_OPCODE_MISC_ALU.0 << 3 | 3;
    instruc_catype(RV32IC_OPCODE_MISC_ALU.1, rs2prime, 0, rdprime, funct6)
}

pub fn rv32ic_lui(rd: i32, imm: i32) -> u32 {
    instruc_citype(
        RV32IC_OPCODE_LUI_ADDI16SP.1,
        rd,
        RV32IC_OPCODE_LUI_ADDI16SP.0,
        imm,
    )
}

pub fn rv32ic_addi16sp(rd: i32, imm: i32) -> u32 {
    let bit_9 = imm >> 9 & 1;
    let bit_4 = imm >> 4 & 1;
    let bit_6 = imm >> 6 & 1;
    let bits_8_to_7 = imm >> 7 & 3;
    let bit_5 = imm >> 5 & 1;
    let imm_o = bit_9 << 5 | bit_4 << 4 | bit_6 << 3 | bits_8_to_7 << 1 | bit_5;
    instruc_citype(
        RV32IC_OPCODE_LUI_ADDI16SP.1,
        rd,
        RV32IC_OPCODE_LUI_ADDI16SP.0,
        imm_o,
    )
}

pub fn rv32ic_addi4spn(rd: i32, imm: i32) -> u32 {
    let rdprime = rd - 8;
    let bits_5_to_4 = imm >> 4 & 3;
    let bits_9_to_6 = imm >> 6 & 15;
    let bit_2 = imm >> 2 & 1;
    let bit_3 = imm >> 3 & 1;
    let imm = bits_5_to_4 << 6 | bits_9_to_6 << 2 | bit_2 << 1 | bit_3;
    instruc_ciwtype(
        RV32IC_OPCODE_ADDI4SPN.1,
        rdprime,
        RV32IC_OPCODE_ADDI4SPN.0,
        imm,
    )
}

pub fn rv32ic_addi(rd: i32, imm: i32) -> u32 {
    instruc_citype(RV32IC_OPCODE_ADDI.1, rd, RV32IC_OPCODE_ADDI.0, imm)
}

pub fn rv32ic_li(rd: i32, imm: i32) -> u32 {
    instruc_citype(RV32IC_OPCODE_LI.1, rd, RV32IC_OPCODE_LI.0, imm)
}

pub fn rv32ic_slli(rd: i32, imm: i32) -> u32 {
    instruc_citype(RV32IC_OPCODE_SLLI.1, rd, RV32IC_OPCODE_SLLI.0, imm)
}

pub fn rv32ic_andi(rs1: i32, imm: i32) -> u32 {
    let rs1prime = rs1 - 8;
    let bit_5 = imm >> 5 & 1;
    let imm = bit_5 << 7 | 2 << 5 | imm & 31;
    instruc_cbtype(
        RV32IC_OPCODE_MISC_ALU.1,
        rs1prime,
        RV32IC_OPCODE_MISC_ALU.0,
        imm,
    )
}

pub fn rv32ic_srli(rs1: i32, imm: i32) -> u32 {
    let rs1prime = rs1 - 8;
    let bit_5 = imm >> 5 & 1;
    let imm = bit_5 << 7 | imm & 31;
    instruc_cbtype(
        RV32IC_OPCODE_MISC_ALU.1,
        rs1prime,
        RV32IC_OPCODE_MISC_ALU.0,
        imm,
    )
}

pub fn rv32ic_srai(rs1: i32, imm: i32) -> u32 {
    let rs1prime = rs1 - 8;
    let bit_5 = imm >> 5 & 1;
    let imm = bit_5 << 7 | 1 << 5 | imm & 31;
    instruc_cbtype(
        RV32IC_OPCODE_MISC_ALU.1,
        rs1prime,
        RV32IC_OPCODE_MISC_ALU.0,
        imm,
    )
}

pub fn rv32ic_beqz(rs1: i32, imm: i32) -> u32 {
    let rs1prime = rs1 - 8;
    let bit_8 = imm >> 8 & 1;
    let bit_4_to_3 = imm >> 3 & 3;
    let bit_7_to_6 = imm >> 6 & 3;
    let bit_2_to_1 = imm >> 1 & 3;
    let bit_5 = imm >> 5 & 1;
    let imm = bit_8 << 7 | bit_4_to_3 << 5 | bit_7_to_6 << 3 | bit_2_to_1 << 1 | bit_5;
    instruc_cbtype(RV32IC_OPCODE_BEQZ.1, rs1prime, RV32IC_OPCODE_BEQZ.0, imm)
}

pub fn rv32ic_bnez(rs1: i32, imm: i32) -> u32 {
    let rs1prime = rs1 - 8;
    let bit_8 = imm >> 8 & 1;
    let bit_4_to_3 = imm >> 3 & 3;
    let bit_7_to_6 = imm >> 6 & 3;
    let bit_2_to_1 = imm >> 1 & 3;
    let bit_5 = imm >> 5 & 1;
    let imm = bit_8 << 7 | bit_4_to_3 << 5 | bit_7_to_6 << 3 | bit_2_to_1 << 1 | bit_5;
    instruc_cbtype(RV32IC_OPCODE_BNEZ.1, rs1prime, RV32IC_OPCODE_BNEZ.0, imm)
}

pub fn rv32ic_jal(imm: i32) -> u32 {
    let bit_11 = imm >> 11 & 1;
    let bit_4 = imm >> 4 & 1;
    let bit_9_to_8 = imm >> 8 & 3;
    let bit_10 = imm >> 10 & 1;
    let bit_6 = imm >> 6 & 1;
    let bit_7 = imm >> 7 & 1;
    let bit_3_to_1 = imm >> 1 & 7;
    let bit_5 = imm >> 5 & 1;
    let imm = bit_11 << 10
        | bit_4 << 9
        | bit_9_to_8 << 7
        | bit_10 << 6
        | bit_6 << 5
        | bit_7 << 4
        | bit_3_to_1 << 1
        | bit_5;
    instruc_cjtype(RV32IC_OPCODE_JAL.1, RV32IC_OPCODE_JAL.0, imm)
}

pub fn rv32ic_j(imm: i32) -> u32 {
    let bit_11 = imm >> 11 & 1;
    let bit_4 = imm >> 4 & 1;
    let bit_9_to_8 = imm >> 8 & 3;
    let bit_10 = imm >> 10 & 1;
    let bit_6 = imm >> 6 & 1;
    let bit_7 = imm >> 7 & 1;
    let bit_3_to_1 = imm >> 1 & 7;
    let bit_5 = imm >> 5 & 1;
    let imm = bit_11 << 10
        | bit_4 << 9
        | bit_9_to_8 << 7
        | bit_10 << 6
        | bit_6 << 5
        | bit_7 << 4
        | bit_3_to_1 << 1
        | bit_5;
    instruc_cjtype(RV32IC_OPCODE_J.1, RV32IC_OPCODE_J.0, imm)
}

pub fn rv32ic_jr(rs1: i32) -> u32 {
    let funct4 = RV32IC_OPCODE_JALR_JR.0 << 1;
    instruc_crtype(RV32IC_OPCODE_JALR_JR.1, 0, rs1, funct4)
}

pub fn rv32ic_jalr(rs1: i32) -> u32 {
    let funct4 = RV32IC_OPCODE_JALR_JR.0 << 1 | 1;
    instruc_crtype(RV32IC_OPCODE_JALR_JR.1, 0, rs1, funct4)
}

pub fn rv32ic_ebreak() -> u32 {
    let funct4 = RV32IC_OPCODE_MISC_ALU.0 << 1 | 1;
    instruc_crtype(RV32IC_OPCODE_MISC_ALU.1, 0, 0, funct4)
}

pub fn rv32ic_lwsp(rd: i32, imm: i32) -> u32 {
    let bit_5 = imm >> 5 & 1;
    let bit_4_to_2 = imm >> 2 & 7;
    let bit_7_to_6 = imm >> 6 & 3;
    let imm = bit_5 << 5 | bit_4_to_2 << 2 | bit_7_to_6;
    instruc_citype(RV32IC_OPCODE_LWSP.1, rd, RV32IC_OPCODE_LWSP.0, imm)
}

pub fn rv32ic_lw(rd: i32, rs1: i32, imm: i32) -> u32 {
    let rdprime = rd - 8;
    let rs1prime = rs1 - 8;
    let bit_5_to_3 = imm >> 3 & 7;
    let bit_2 = imm >> 2 & 1;
    let bit_6 = imm >> 6 & 1;
    let imm = bit_5_to_3 << 2 | bit_2 << 1 | bit_6;
    instruc_cltype(
        RV32IC_OPCODE_LW.1,
        rdprime,
        rs1prime,
        RV32IC_OPCODE_LW.0,
        imm,
    )
}

pub fn rv32ic_swsp(rs2: i32, imm: i32) -> u32 {
    let bit_5_to_2 = imm >> 2 & 15;
    let bit_7_to_6 = imm >> 6 & 3;
    let imm = bit_5_to_2 << 2 | bit_7_to_6;
    instruc_csstype(RV32IC_OPCODE_SWSP.1, rs2, RV32IC_OPCODE_SWSP.0, imm)
}

pub fn rv32ic_sw(rs1: i32, rs2: i32, imm: i32) -> u32 {
    let rs1prime = rs1 - 8;
    let rs2prime = rs2 - 8;
    let bit_5_to_3 = imm >> 3 & 7;
    let bit_2 = imm >> 2 & 1;
    let bit_6 = imm >> 6 & 1;
    let imm = bit_5_to_3 << 2 | bit_2 << 1 | bit_6;
    instruc_cstype(
        RV32IC_OPCODE_SW.1,
        rs1prime,
        rs2prime,
        RV32IC_OPCODE_SW.0,
        imm,
    )
}
