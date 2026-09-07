// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

//! Port of `riscv/rvprotoinstrs.py`; function names and argument order are retained.

pub fn instruc_rtype(opcode: i32, rd: i32, funct3: i32, rs1: i32, rs2: i32, funct7: i32) -> u32 {
    assert!(opcode >= 0);
    assert!(rd >= 0);
    assert!(funct3 >= 0);
    assert!(rs1 >= 0);
    assert!(rs2 >= 0);
    assert!(funct7 >= 0);
    assert!(opcode < 1 << 7);
    assert!(rd < 32);
    assert!(funct3 < 8);
    assert!(rs1 < 32);
    assert!(rs2 < 32);
    assert!(funct7 < 1 << 7);
    let rd_offset = 7;
    let funct3_offset = rd_offset + 5;
    let rs1_offset = funct3_offset + 3;
    let rs2_offset = rs1_offset + 5;
    let funct7_offset = rs2_offset + 5;
    (opcode
        | rd << rd_offset
        | funct3 << funct3_offset
        | rs1 << rs1_offset
        | rs2 << rs2_offset
        | funct7 << funct7_offset) as u32
}

pub fn instruc_itype(opcode: i32, rd: i32, funct3: i32, rs1: i32, imm: i32) -> u32 {
    assert!(opcode < 1 << 7);
    assert!(rd < 32);
    assert!(funct3 < 8);
    assert!(rs1 < 32);
    assert!(opcode >= 0);
    assert!(rd >= 0);
    assert!(funct3 >= 0);
    assert!(rs1 >= 0);
    let imm = imm & 4095;
    let rd_offset = 7;
    let funct3_offset = rd_offset + 5;
    let rs1_offset = funct3_offset + 3;
    let imm_offset = rs1_offset + 5;
    (opcode | rd << rd_offset | funct3 << funct3_offset | rs1 << rs1_offset | imm << imm_offset)
        as u32
}

pub fn instruc_stype(opcode: i32, funct3: i32, rs1: i32, rs2: i32, imm: i32) -> u32 {
    assert!(opcode < 1 << 7);
    assert!(funct3 < 8);
    assert!(rs1 < 32);
    assert!(rs2 < 32);
    assert!(opcode >= 0);
    assert!(funct3 >= 0);
    assert!(rs1 >= 0);
    assert!(rs2 >= 0);
    let imm = imm & 4095;
    let imm4_0_offset = 7;
    let funct3_offset = imm4_0_offset + 5;
    let rs1_offset = funct3_offset + 3;
    let rs2_offset = rs1_offset + 5;
    let imm11_5_offset = rs2_offset + 5;
    let imm4_0 = imm & 31;
    let imm11_5 = imm >> 5 & 127;
    (opcode
        | imm4_0 << imm4_0_offset
        | funct3 << funct3_offset
        | rs1 << rs1_offset
        | rs2 << rs2_offset
        | imm11_5 << imm11_5_offset) as u32
}

pub fn instruc_btype(opcode: i32, funct3: i32, rs1: i32, rs2: i32, imm: i32) -> u32 {
    assert!(opcode < 1 << 7);
    assert!(funct3 < 8);
    assert!(rs1 < 32);
    assert!(rs2 < 32);
    assert!(opcode >= 0);
    assert!(funct3 >= 0);
    assert!(rs1 >= 0);
    assert!(rs2 >= 0);
    let imm = imm & 8191;
    let imm4_1_11_offset = 7;
    let funct3_offset = imm4_1_11_offset + 5;
    let rs1_offset = funct3_offset + 3;
    let rs2_offset = rs1_offset + 5;
    let imm12_10_5_offset = rs2_offset + 5;
    let imm4_1_11 = imm >> 11 & 1 | (imm >> 1 & 15) << 1;
    let imm12_10_5 = imm >> 5 & 63 | (imm >> 12 & 1) << 6;
    (opcode
        | imm4_1_11 << imm4_1_11_offset
        | funct3 << funct3_offset
        | rs1 << rs1_offset
        | rs2 << rs2_offset
        | imm12_10_5 << imm12_10_5_offset) as u32
}

pub fn instruc_utype(opcode: i32, rd: i32, imm: i32) -> u32 {
    assert!(opcode < 1 << 7);
    assert!(rd < 32);
    assert!(opcode >= 0);
    assert!(rd >= 0);
    let rd_offset = 7;
    let imm31_12_offset = rd_offset + 5;
    let imm31_12 = imm & 1048575;
    (opcode | rd << rd_offset | imm31_12 << imm31_12_offset) as u32
}

pub fn instruc_jtype(opcode: i32, rd: i32, imm: i32) -> u32 {
    assert!(opcode < 1 << 7);
    assert!(rd < 32);
    assert!(opcode >= 0);
    assert!(rd >= 0);
    let rd_offset = 7;
    let immparts_offset = rd_offset + 5;
    let immparts =
        imm >> 12 & 255 | (imm >> 11 & 1) << 8 | (imm >> 1 & 1023) << 9 | (imm >> 20 & 1) << 19;
    (opcode | rd << rd_offset | immparts << immparts_offset) as u32
}

pub fn instruc_r4type(
    opcode: i32,
    rd: i32,
    funct3: i32,
    rs1: i32,
    rs2: i32,
    rs3: i32,
    funct2: i32,
) -> u32 {
    assert!(opcode < 1 << 7);
    assert!(rd < 32);
    assert!(funct3 < 8);
    assert!(rs1 < 32);
    assert!(rs2 < 32);
    assert!(rs3 < 32);
    assert!(funct2 < 1 << 2);
    assert!(opcode >= 0);
    assert!(rd >= 0);
    assert!(funct3 >= 0);
    assert!(rs1 >= 0);
    assert!(rs2 >= 0);
    assert!(rs3 >= 0);
    assert!(funct2 >= 0);
    let rd_offset = 7;
    let funct3_offset = rd_offset + 5;
    let rs1_offset = funct3_offset + 3;
    let rs2_offset = rs1_offset + 5;
    let funct2_offset = rs2_offset + 5;
    let rs3_offset = funct2_offset + 2;
    (opcode
        | rd << rd_offset
        | funct3 << funct3_offset
        | rs1 << rs1_offset
        | rs2 << rs2_offset
        | funct2 << funct2_offset
        | rs3 << rs3_offset) as u32
}

pub fn instruc_crtype(opcode: i32, rs2: i32, rds1: i32, funct4: i32) -> u32 {
    assert!(opcode < 4);
    assert!(rs2 < 32);
    assert!(rds1 < 32);
    assert!(funct4 < 1 << 4);
    assert!(opcode >= 0);
    assert!(rs2 >= 0);
    assert!(rds1 >= 0);
    assert!(funct4 >= 0);
    let rs2_offset = 2;
    let rds1_offset = rs2_offset + 5;
    let funct4_offset = rds1_offset + 5;
    (opcode | rs2 << rs2_offset | rds1 << rds1_offset | funct4 << funct4_offset) as u32
}

pub fn instruc_citype(opcode: i32, rds1: i32, funct3: i32, imm: i32) -> u32 {
    assert!(opcode < 4);
    assert!(rds1 < 32);
    assert!(funct3 < 1 << 3);
    let imm4_0_offset = 2;
    let rds1_offset = imm4_0_offset + 5;
    let imm5_offset = rds1_offset + 5;
    let funct3_offset = imm5_offset + 1;
    let imm4_0 = imm & 31;
    let imm5 = imm >> 5 & 1;
    (opcode
        | imm4_0 << imm4_0_offset
        | rds1 << rds1_offset
        | imm5 << imm5_offset
        | funct3 << funct3_offset) as u32
}

pub fn instruc_csstype(opcode: i32, rs2: i32, funct3: i32, imm: i32) -> u32 {
    assert!(opcode < 4);
    assert!(rs2 < 32);
    assert!(funct3 < 1 << 3);
    assert!(imm < 1 << 7);
    let rs2_offset = 2;
    let imm_offset = rs2_offset + 5;
    let funct3_offset = imm_offset + 6;
    (opcode | rs2 << rs2_offset | imm << imm_offset | funct3 << funct3_offset) as u32
}

pub fn instruc_ciwtype(opcode: i32, rdprime: i32, funct3: i32, imm: i32) -> u32 {
    assert!(opcode < 4);
    assert!(rdprime < 8);
    assert!(funct3 < 1 << 3);
    assert!(imm < 1 << 9);
    let rdprime_offset = 2;
    let imm_offset = rdprime_offset + 3;
    let funct3_offset = imm_offset + 9;
    (opcode | rdprime << rdprime_offset | imm << imm_offset | funct3 << funct3_offset) as u32
}

pub fn instruc_cltype(opcode: i32, rdprime: i32, rs1prime: i32, funct3: i32, imm: i32) -> u32 {
    assert!(opcode < 4);
    assert!(rdprime < 8);
    assert!(rs1prime < 8);
    assert!(funct3 < 1 << 3);
    assert!(imm < 1 << 5);
    let rdprime_offset = 2;
    let imm1_0_offset = rdprime_offset + 3;
    let rs1prime_offset = imm1_0_offset + 2;
    let imm4_2_offset = rs1prime_offset + 3;
    let funct3_offset = imm4_2_offset + 3;
    let imm1_0 = imm & 3;
    let imm4_2 = imm >> 2 & 7;
    (opcode
        | rdprime << rdprime_offset
        | imm1_0 << imm1_0_offset
        | rs1prime << rs1prime_offset
        | imm4_2 << imm4_2_offset
        | funct3 << funct3_offset) as u32
}

pub fn instruc_cstype(opcode: i32, rs1prime: i32, rs2prime: i32, funct3: i32, imm: i32) -> u32 {
    assert!(opcode < 4);
    assert!(rs1prime < 8);
    assert!(rs2prime < 8);
    assert!(funct3 < 1 << 3);
    assert!(imm < 1 << 5);
    let rs2prime_offset = 2;
    let imm1_0_offset = rs2prime_offset + 3;
    let rs1prime_offset = imm1_0_offset + 2;
    let imm4_2_offset = rs1prime_offset + 3;
    let funct3_offset = imm4_2_offset + 3;
    let imm1_0 = imm & 3;
    let imm4_2 = imm >> 2 & 7;
    (opcode
        | rs2prime << rs2prime_offset
        | imm1_0 << imm1_0_offset
        | rs1prime << rs1prime_offset
        | imm4_2 << imm4_2_offset
        | funct3 << funct3_offset) as u32
}

pub fn instruc_catype(opcode: i32, rs2prime: i32, funct2: i32, rds1prime: i32, funct6: i32) -> u32 {
    assert!(opcode < 4);
    assert!(rds1prime < 8);
    assert!(rs2prime < 8);
    assert!(funct2 < 4);
    assert!(funct6 < 1 << 6);
    let rs2prime_offset = 2;
    let funct2_offset = rs2prime_offset + 3;
    let rds1prime_offset = funct2_offset + 2;
    let funct6_offset = rds1prime_offset + 3;
    (opcode
        | rs2prime << rs2prime_offset
        | funct2 << funct2_offset
        | rds1prime << rds1prime_offset
        | funct6 << funct6_offset) as u32
}

pub fn instruc_cbtype(opcode: i32, rs1prime: i32, funct3: i32, imm: i32) -> u32 {
    assert!(opcode < 4);
    assert!(rs1prime < 8);
    assert!(funct3 < 8);
    let imm4_0_offset = 2;
    let rs1prime_offset = imm4_0_offset + 5;
    let imm7_5_offset = rs1prime_offset + 3;
    let funct3_offset = imm7_5_offset + 3;
    let imm4_0 = imm & 31;
    let imm7_5 = imm >> 5 & 7;
    (opcode
        | imm4_0 << imm4_0_offset
        | rs1prime << rs1prime_offset
        | imm7_5 << imm7_5_offset
        | funct3 << funct3_offset) as u32
}

pub fn instruc_cjtype(opcode: i32, funct3: i32, imm: i32) -> u32 {
    assert!(opcode < 4);
    assert!(funct3 < 8);
    assert!(imm < 1 << 11);
    let imm_offset = 2;
    let funct3_offset = imm_offset + 11;
    (opcode | imm << imm_offset | funct3 << funct3_offset) as u32
}
