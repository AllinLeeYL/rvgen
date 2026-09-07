// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

//! Port of `riscv/rv32a.py`; function names and argument order are retained.

use super::rvprotoinstrs::*;

pub const RV32A_OPCODE_AMO: i32 = 47;

pub fn rv32a_lrw(aq: bool, rl: bool, rd: i32, rs1: i32, rs2: i32) -> u32 {
    let funct3 = 2;
    let funct7 = 2 << 2 | (aq as i32) << 1 | (rl as i32);
    instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

pub fn rv32a_scw(aq: bool, rl: bool, rd: i32, rs1: i32, rs2: i32) -> u32 {
    let funct3 = 2;
    let funct7 = 3 << 2 | (aq as i32) << 1 | (rl as i32);
    instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

pub fn rv32a_amoswapw(aq: bool, rl: bool, rd: i32, rs1: i32, rs2: i32) -> u32 {
    let funct3 = 2;
    let funct7 = 1 << 2 | (aq as i32) << 1 | (rl as i32);
    instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

pub fn rv32a_amoaddw(aq: bool, rl: bool, rd: i32, rs1: i32, rs2: i32) -> u32 {
    let funct3 = 2;
    let funct7 = (aq as i32) << 1 | (rl as i32);
    instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

pub fn rv32a_amoandw(aq: bool, rl: bool, rd: i32, rs1: i32, rs2: i32) -> u32 {
    let funct3 = 2;
    let funct7 = 12 << 2 | (aq as i32) << 1 | (rl as i32);
    instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

pub fn rv32a_amoorw(aq: bool, rl: bool, rd: i32, rs1: i32, rs2: i32) -> u32 {
    let funct3 = 2;
    let funct7 = 8 << 2 | (aq as i32) << 1 | (rl as i32);
    instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

pub fn rv32a_amoxorw(aq: bool, rl: bool, rd: i32, rs1: i32, rs2: i32) -> u32 {
    let funct3 = 2;
    let funct7 = 4 << 2 | (aq as i32) << 1 | (rl as i32);
    instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

pub fn rv32a_amomaxw(aq: bool, rl: bool, rd: i32, rs1: i32, rs2: i32) -> u32 {
    let funct3 = 2;
    let funct7 = 20 << 2 | (aq as i32) << 1 | (rl as i32);
    instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

pub fn rv32a_amomaxuw(aq: bool, rl: bool, rd: i32, rs1: i32, rs2: i32) -> u32 {
    let funct3 = 2;
    let funct7 = 28 << 2 | (aq as i32) << 1 | (rl as i32);
    instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

pub fn rv32a_amominw(aq: bool, rl: bool, rd: i32, rs1: i32, rs2: i32) -> u32 {
    let funct3 = 2;
    let funct7 = 16 << 2 | (aq as i32) << 1 | (rl as i32);
    instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

pub fn rv32a_amominuw(aq: bool, rl: bool, rd: i32, rs1: i32, rs2: i32) -> u32 {
    let funct3 = 2;
    let funct7 = 24 << 2 | (aq as i32) << 1 | (rl as i32);
    instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}
