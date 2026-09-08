// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

package riscv

// Bit encoders preserve the original argument order and immediate masking.
var RV64IC_OPCODE_MISC_ALU = [2]int32{4, 1}
var RV64IC_OPCODE_ADDIW = [2]int32{1, 1}
var RV64IC_OPCODE_LDSP = [2]int32{3, 2}
var RV64IC_OPCODE_LD = [2]int32{3, 0}
var RV64IC_OPCODE_SDSP = [2]int32{7, 2}
var RV64IC_OPCODE_SD = [2]int32{7, 0}

func Rv64ic_addw(rd int32, rs2 int32) uint32 {
	var rdprime int32 = rd - 8
	var rs2prime int32 = rs2 - 8
	var funct6 int32 = RV64IC_OPCODE_MISC_ALU[0]<<3 | 7
	return Instruc_catype(RV64IC_OPCODE_MISC_ALU[1], rs2prime, 1, rdprime, funct6)
}

func Rv64ic_subw(rd int32, rs2 int32) uint32 {
	var rdprime int32 = rd - 8
	var rs2prime int32 = rs2 - 8
	var funct6 int32 = RV64IC_OPCODE_MISC_ALU[0]<<3 | 7
	return Instruc_catype(RV64IC_OPCODE_MISC_ALU[1], rs2prime, 0, rdprime, funct6)
}

func Rv64ic_addiw(rd int32, imm int32) uint32 {
	return Instruc_citype(RV64IC_OPCODE_ADDIW[1], rd, RV64IC_OPCODE_ADDIW[0], imm)
}

func Rv64ic_ldsp(rd int32, imm int32) uint32 {
	var bit_5 int32 = imm >> 5 & 1
	var bit_4_to_3 int32 = imm >> 3 & 3
	var bit_8_to_6 int32 = imm >> 6 & 7
	imm = bit_5<<5 | bit_4_to_3<<3 | bit_8_to_6
	return Instruc_citype(RV64IC_OPCODE_LDSP[1], rd, RV64IC_OPCODE_LDSP[0], imm)
}

func Rv64ic_ld(rd int32, rs1 int32, imm int32) uint32 {
	var rdprime int32 = rd - 8
	var rs1prime int32 = rs1 - 8
	var bit_5_to_3 int32 = imm >> 3 & 7
	var bit_7_to_6 int32 = imm >> 6 & 3
	imm = bit_5_to_3<<2 | bit_7_to_6
	return Instruc_cltype(RV64IC_OPCODE_LD[1], rdprime, rs1prime, RV64IC_OPCODE_LD[0], imm)
}

func Rv64ic_sdsp(rs2 int32, imm int32) uint32 {
	var bit_5_to_3 int32 = imm >> 2 & 7
	var bit_8_to_6 int32 = imm >> 6 & 7
	imm = bit_5_to_3<<3 | bit_8_to_6
	return Instruc_csstype(RV64IC_OPCODE_SDSP[1], rs2, RV64IC_OPCODE_SDSP[0], imm)
}

func Rv64ic_sd(rs1 int32, rs2 int32, imm int32) uint32 {
	var rs1prime int32 = rs1 - 8
	var rs2prime int32 = rs2 - 8
	var bit_5_to_3 int32 = imm >> 3 & 7
	var bit_7_to_6 int32 = imm >> 6 & 3
	imm = bit_5_to_3<<2 | bit_7_to_6
	return Instruc_cstype(RV64IC_OPCODE_SD[1], rs1prime, rs2prime, RV64IC_OPCODE_SD[0], imm)
}
