// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

package riscv

// Bit encoders preserve the original argument order and immediate masking.
var RV32IC_OPCODE_MV_ADD = [2]int32{4, 2}
var RV32IC_OPCODE_MISC_ALU = [2]int32{4, 1}
var RV32IC_OPCODE_LUI_ADDI16SP = [2]int32{3, 1}
var RV32IC_OPCODE_ADDI4SPN = [2]int32{0, 0}
var RV32IC_OPCODE_ADDI = [2]int32{0, 1}
var RV32IC_OPCODE_LI = [2]int32{2, 1}
var RV32IC_OPCODE_SLLI = [2]int32{0, 2}
var RV32IC_OPCODE_BEQZ = [2]int32{6, 1}
var RV32IC_OPCODE_BNEZ = [2]int32{7, 1}
var RV32IC_OPCODE_J = [2]int32{5, 1}
var RV32IC_OPCODE_JAL = [2]int32{1, 1}
var RV32IC_OPCODE_JALR_JR = [2]int32{4, 2}
var RV32IC_OPCODE_LWSP = [2]int32{2, 2}
var RV32IC_OPCODE_LW = [2]int32{2, 0}
var RV32IC_OPCODE_SWSP = [2]int32{6, 2}
var RV32IC_OPCODE_SW = [2]int32{6, 0}

func Rv32ic_mv(rd int32, rs2 int32) uint32 {
	var funct4 int32 = RV32IC_OPCODE_MV_ADD[0] << 1
	return Instruc_crtype(RV32IC_OPCODE_MV_ADD[1], rs2, rd, funct4)
}

func Rv32ic_add(rd int32, rs2 int32) uint32 {
	var funct4 int32 = RV32IC_OPCODE_MV_ADD[0]<<1 | 1
	return Instruc_crtype(RV32IC_OPCODE_MV_ADD[1], rs2, rd, funct4)
}

func Rv32ic_and(rd int32, rs2 int32) uint32 {
	var rdprime int32 = rd - 8
	var rs2prime int32 = rs2 - 8
	var funct6 int32 = RV32IC_OPCODE_MISC_ALU[0]<<3 | 3
	return Instruc_catype(RV32IC_OPCODE_MISC_ALU[1], rs2prime, 3, rdprime, funct6)
}

func Rv32ic_or(rd int32, rs2 int32) uint32 {
	var rdprime int32 = rd - 8
	var rs2prime int32 = rs2 - 8
	var funct6 int32 = RV32IC_OPCODE_MISC_ALU[0]<<3 | 3
	return Instruc_catype(RV32IC_OPCODE_MISC_ALU[1], rs2prime, 2, rdprime, funct6)
}

func Rv32ic_xor(rd int32, rs2 int32) uint32 {
	var rdprime int32 = rd - 8
	var rs2prime int32 = rs2 - 8
	var funct6 int32 = RV32IC_OPCODE_MISC_ALU[0]<<3 | 3
	return Instruc_catype(RV32IC_OPCODE_MISC_ALU[1], rs2prime, 1, rdprime, funct6)
}

func Rv32ic_sub(rd int32, rs2 int32) uint32 {
	var rdprime int32 = rd - 8
	var rs2prime int32 = rs2 - 8
	var funct6 int32 = RV32IC_OPCODE_MISC_ALU[0]<<3 | 3
	return Instruc_catype(RV32IC_OPCODE_MISC_ALU[1], rs2prime, 0, rdprime, funct6)
}

func Rv32ic_lui(rd int32, imm int32) uint32 {
	return Instruc_citype(RV32IC_OPCODE_LUI_ADDI16SP[1], rd, RV32IC_OPCODE_LUI_ADDI16SP[0], imm)
}

func Rv32ic_addi16sp(rd int32, imm int32) uint32 {
	var bit_9 int32 = imm >> 9 & 1
	var bit_4 int32 = imm >> 4 & 1
	var bit_6 int32 = imm >> 6 & 1
	var bits_8_to_7 int32 = imm >> 7 & 3
	var bit_5 int32 = imm >> 5 & 1
	var imm_o int32 = bit_9<<5 | bit_4<<4 | bit_6<<3 | bits_8_to_7<<1 | bit_5
	return Instruc_citype(RV32IC_OPCODE_LUI_ADDI16SP[1], rd, RV32IC_OPCODE_LUI_ADDI16SP[0], imm_o)
}

func Rv32ic_addi4spn(rd int32, imm int32) uint32 {
	var rdprime int32 = rd - 8
	var bits_5_to_4 int32 = imm >> 4 & 3
	var bits_9_to_6 int32 = imm >> 6 & 15
	var bit_2 int32 = imm >> 2 & 1
	var bit_3 int32 = imm >> 3 & 1
	imm = bits_5_to_4<<6 | bits_9_to_6<<2 | bit_2<<1 | bit_3
	return Instruc_ciwtype(RV32IC_OPCODE_ADDI4SPN[1], rdprime, RV32IC_OPCODE_ADDI4SPN[0], imm)
}

func Rv32ic_addi(rd int32, imm int32) uint32 {
	return Instruc_citype(RV32IC_OPCODE_ADDI[1], rd, RV32IC_OPCODE_ADDI[0], imm)
}

func Rv32ic_li(rd int32, imm int32) uint32 {
	return Instruc_citype(RV32IC_OPCODE_LI[1], rd, RV32IC_OPCODE_LI[0], imm)
}

func Rv32ic_slli(rd int32, imm int32) uint32 {
	return Instruc_citype(RV32IC_OPCODE_SLLI[1], rd, RV32IC_OPCODE_SLLI[0], imm)
}

func Rv32ic_andi(rs1 int32, imm int32) uint32 {
	var rs1prime int32 = rs1 - 8
	var bit_5 int32 = imm >> 5 & 1
	imm = bit_5<<7 | 2<<5 | imm&31
	return Instruc_cbtype(RV32IC_OPCODE_MISC_ALU[1], rs1prime, RV32IC_OPCODE_MISC_ALU[0], imm)
}

func Rv32ic_srli(rs1 int32, imm int32) uint32 {
	var rs1prime int32 = rs1 - 8
	var bit_5 int32 = imm >> 5 & 1
	imm = bit_5<<7 | imm&31
	return Instruc_cbtype(RV32IC_OPCODE_MISC_ALU[1], rs1prime, RV32IC_OPCODE_MISC_ALU[0], imm)
}

func Rv32ic_srai(rs1 int32, imm int32) uint32 {
	var rs1prime int32 = rs1 - 8
	var bit_5 int32 = imm >> 5 & 1
	imm = bit_5<<7 | 1<<5 | imm&31
	return Instruc_cbtype(RV32IC_OPCODE_MISC_ALU[1], rs1prime, RV32IC_OPCODE_MISC_ALU[0], imm)
}

func Rv32ic_beqz(rs1 int32, imm int32) uint32 {
	var rs1prime int32 = rs1 - 8
	var bit_8 int32 = imm >> 8 & 1
	var bit_4_to_3 int32 = imm >> 3 & 3
	var bit_7_to_6 int32 = imm >> 6 & 3
	var bit_2_to_1 int32 = imm >> 1 & 3
	var bit_5 int32 = imm >> 5 & 1
	imm = bit_8<<7 | bit_4_to_3<<5 | bit_7_to_6<<3 | bit_2_to_1<<1 | bit_5
	return Instruc_cbtype(RV32IC_OPCODE_BEQZ[1], rs1prime, RV32IC_OPCODE_BEQZ[0], imm)
}

func Rv32ic_bnez(rs1 int32, imm int32) uint32 {
	var rs1prime int32 = rs1 - 8
	var bit_8 int32 = imm >> 8 & 1
	var bit_4_to_3 int32 = imm >> 3 & 3
	var bit_7_to_6 int32 = imm >> 6 & 3
	var bit_2_to_1 int32 = imm >> 1 & 3
	var bit_5 int32 = imm >> 5 & 1
	imm = bit_8<<7 | bit_4_to_3<<5 | bit_7_to_6<<3 | bit_2_to_1<<1 | bit_5
	return Instruc_cbtype(RV32IC_OPCODE_BNEZ[1], rs1prime, RV32IC_OPCODE_BNEZ[0], imm)
}

func Rv32ic_jal(imm int32) uint32 {
	var bit_11 int32 = imm >> 11 & 1
	var bit_4 int32 = imm >> 4 & 1
	var bit_9_to_8 int32 = imm >> 8 & 3
	var bit_10 int32 = imm >> 10 & 1
	var bit_6 int32 = imm >> 6 & 1
	var bit_7 int32 = imm >> 7 & 1
	var bit_3_to_1 int32 = imm >> 1 & 7
	var bit_5 int32 = imm >> 5 & 1
	imm = bit_11<<10 | bit_4<<9 | bit_9_to_8<<7 | bit_10<<6 | bit_6<<5 | bit_7<<4 | bit_3_to_1<<1 | bit_5
	return Instruc_cjtype(RV32IC_OPCODE_JAL[1], RV32IC_OPCODE_JAL[0], imm)
}

func Rv32ic_j(imm int32) uint32 {
	var bit_11 int32 = imm >> 11 & 1
	var bit_4 int32 = imm >> 4 & 1
	var bit_9_to_8 int32 = imm >> 8 & 3
	var bit_10 int32 = imm >> 10 & 1
	var bit_6 int32 = imm >> 6 & 1
	var bit_7 int32 = imm >> 7 & 1
	var bit_3_to_1 int32 = imm >> 1 & 7
	var bit_5 int32 = imm >> 5 & 1
	imm = bit_11<<10 | bit_4<<9 | bit_9_to_8<<7 | bit_10<<6 | bit_6<<5 | bit_7<<4 | bit_3_to_1<<1 | bit_5
	return Instruc_cjtype(RV32IC_OPCODE_J[1], RV32IC_OPCODE_J[0], imm)
}

func Rv32ic_jr(rs1 int32) uint32 {
	var funct4 int32 = RV32IC_OPCODE_JALR_JR[0] << 1
	return Instruc_crtype(RV32IC_OPCODE_JALR_JR[1], 0, rs1, funct4)
}

func Rv32ic_jalr(rs1 int32) uint32 {
	var funct4 int32 = RV32IC_OPCODE_JALR_JR[0]<<1 | 1
	return Instruc_crtype(RV32IC_OPCODE_JALR_JR[1], 0, rs1, funct4)
}

func Rv32ic_ebreak() uint32 {
	var funct4 int32 = RV32IC_OPCODE_MISC_ALU[0]<<1 | 1
	return Instruc_crtype(RV32IC_OPCODE_MISC_ALU[1], 0, 0, funct4)
}

func Rv32ic_lwsp(rd int32, imm int32) uint32 {
	var bit_5 int32 = imm >> 5 & 1
	var bit_4_to_2 int32 = imm >> 2 & 7
	var bit_7_to_6 int32 = imm >> 6 & 3
	imm = bit_5<<5 | bit_4_to_2<<2 | bit_7_to_6
	return Instruc_citype(RV32IC_OPCODE_LWSP[1], rd, RV32IC_OPCODE_LWSP[0], imm)
}

func Rv32ic_lw(rd int32, rs1 int32, imm int32) uint32 {
	var rdprime int32 = rd - 8
	var rs1prime int32 = rs1 - 8
	var bit_5_to_3 int32 = imm >> 3 & 7
	var bit_2 int32 = imm >> 2 & 1
	var bit_6 int32 = imm >> 6 & 1
	imm = bit_5_to_3<<2 | bit_2<<1 | bit_6
	return Instruc_cltype(RV32IC_OPCODE_LW[1], rdprime, rs1prime, RV32IC_OPCODE_LW[0], imm)
}

func Rv32ic_swsp(rs2 int32, imm int32) uint32 {
	var bit_5_to_2 int32 = imm >> 2 & 15
	var bit_7_to_6 int32 = imm >> 6 & 3
	imm = bit_5_to_2<<2 | bit_7_to_6
	return Instruc_csstype(RV32IC_OPCODE_SWSP[1], rs2, RV32IC_OPCODE_SWSP[0], imm)
}

func Rv32ic_sw(rs1 int32, rs2 int32, imm int32) uint32 {
	var rs1prime int32 = rs1 - 8
	var rs2prime int32 = rs2 - 8
	var bit_5_to_3 int32 = imm >> 3 & 7
	var bit_2 int32 = imm >> 2 & 1
	var bit_6 int32 = imm >> 6 & 1
	imm = bit_5_to_3<<2 | bit_2<<1 | bit_6
	return Instruc_cstype(RV32IC_OPCODE_SW[1], rs1prime, rs2prime, RV32IC_OPCODE_SW[0], imm)
}
