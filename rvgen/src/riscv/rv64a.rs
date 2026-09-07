// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

//! Port of `riscv/rv64a.py`; function names and argument order are retained.

use super::rvprotoinstrs::*;

pub const RV64A_OPCODE_AMO: i32 = 47;

pub fn rv64a_lrd(aq: bool, rl: bool, rd: i32, rs1: i32, rs2: i32) -> u32 {
    let funct3 = 3;
    let funct7 = 2 << 2 | (aq as i32) << 1 | (rl as i32);
    instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

pub fn rv64a_scd(aq: bool, rl: bool, rd: i32, rs1: i32, rs2: i32) -> u32 {
    let funct3 = 3;
    let funct7 = 3 << 2 | (aq as i32) << 1 | (rl as i32);
    instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

pub fn rv64a_amoswapd(aq: bool, rl: bool, rd: i32, rs1: i32, rs2: i32) -> u32 {
    let funct3 = 3;
    let funct7 = 1 << 2 | (aq as i32) << 1 | (rl as i32);
    instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

pub fn rv64a_amoaddd(aq: bool, rl: bool, rd: i32, rs1: i32, rs2: i32) -> u32 {
    let funct3 = 3;
    let funct7 = (aq as i32) << 1 | (rl as i32);
    instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

pub fn rv64a_amoandd(aq: bool, rl: bool, rd: i32, rs1: i32, rs2: i32) -> u32 {
    let funct3 = 3;
    let funct7 = 12 << 2 | (aq as i32) << 1 | (rl as i32);
    instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

pub fn rv64a_amoord(aq: bool, rl: bool, rd: i32, rs1: i32, rs2: i32) -> u32 {
    let funct3 = 3;
    let funct7 = 8 << 2 | (aq as i32) << 1 | (rl as i32);
    instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

pub fn rv64a_amoxord(aq: bool, rl: bool, rd: i32, rs1: i32, rs2: i32) -> u32 {
    let funct3 = 3;
    let funct7 = 4 << 2 | (aq as i32) << 1 | (rl as i32);
    instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

pub fn rv64a_amomaxd(aq: bool, rl: bool, rd: i32, rs1: i32, rs2: i32) -> u32 {
    let funct3 = 3;
    let funct7 = 20 << 2 | (aq as i32) << 1 | (rl as i32);
    instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

pub fn rv64a_amomaxud(aq: bool, rl: bool, rd: i32, rs1: i32, rs2: i32) -> u32 {
    let funct3 = 3;
    let funct7 = 28 << 2 | (aq as i32) << 1 | (rl as i32);
    instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

pub fn rv64a_amomind(aq: bool, rl: bool, rd: i32, rs1: i32, rs2: i32) -> u32 {
    let funct3 = 3;
    let funct7 = 16 << 2 | (aq as i32) << 1 | (rl as i32);
    instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

pub fn rv64a_amominud(aq: bool, rl: bool, rd: i32, rs1: i32, rs2: i32) -> u32 {
    let funct3 = 3;
    let funct7 = 24 << 2 | (aq as i32) << 1 | (rl as i32);
    instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}
