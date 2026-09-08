// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

package riscv

// Bit encoders preserve the original argument order and immediate masking.
const RV64A_OPCODE_AMO int32 = 47

func Rv64a_lrd(aq bool, rl bool, rd int32, rs1 int32, rs2 int32) uint32 {
	var funct3 int32 = 3
	var funct7 int32 = 2<<2 | boolInt(aq)<<1 | boolInt(rl)
	return Instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

func Rv64a_scd(aq bool, rl bool, rd int32, rs1 int32, rs2 int32) uint32 {
	var funct3 int32 = 3
	var funct7 int32 = 3<<2 | boolInt(aq)<<1 | boolInt(rl)
	return Instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

func Rv64a_amoswapd(aq bool, rl bool, rd int32, rs1 int32, rs2 int32) uint32 {
	var funct3 int32 = 3
	var funct7 int32 = 1<<2 | boolInt(aq)<<1 | boolInt(rl)
	return Instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

func Rv64a_amoaddd(aq bool, rl bool, rd int32, rs1 int32, rs2 int32) uint32 {
	var funct3 int32 = 3
	var funct7 int32 = boolInt(aq)<<1 | boolInt(rl)
	return Instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

func Rv64a_amoandd(aq bool, rl bool, rd int32, rs1 int32, rs2 int32) uint32 {
	var funct3 int32 = 3
	var funct7 int32 = 12<<2 | boolInt(aq)<<1 | boolInt(rl)
	return Instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

func Rv64a_amoord(aq bool, rl bool, rd int32, rs1 int32, rs2 int32) uint32 {
	var funct3 int32 = 3
	var funct7 int32 = 8<<2 | boolInt(aq)<<1 | boolInt(rl)
	return Instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

func Rv64a_amoxord(aq bool, rl bool, rd int32, rs1 int32, rs2 int32) uint32 {
	var funct3 int32 = 3
	var funct7 int32 = 4<<2 | boolInt(aq)<<1 | boolInt(rl)
	return Instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

func Rv64a_amomaxd(aq bool, rl bool, rd int32, rs1 int32, rs2 int32) uint32 {
	var funct3 int32 = 3
	var funct7 int32 = 20<<2 | boolInt(aq)<<1 | boolInt(rl)
	return Instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

func Rv64a_amomaxud(aq bool, rl bool, rd int32, rs1 int32, rs2 int32) uint32 {
	var funct3 int32 = 3
	var funct7 int32 = 28<<2 | boolInt(aq)<<1 | boolInt(rl)
	return Instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

func Rv64a_amomind(aq bool, rl bool, rd int32, rs1 int32, rs2 int32) uint32 {
	var funct3 int32 = 3
	var funct7 int32 = 16<<2 | boolInt(aq)<<1 | boolInt(rl)
	return Instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

func Rv64a_amominud(aq bool, rl bool, rd int32, rs1 int32, rs2 int32) uint32 {
	var funct3 int32 = 3
	var funct7 int32 = 24<<2 | boolInt(aq)<<1 | boolInt(rl)
	return Instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}
