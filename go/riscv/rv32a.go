// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

package riscv

// Bit encoders preserve the original argument order and immediate masking.
const RV32A_OPCODE_AMO int32 = 47

func Rv32a_lrw(aq bool, rl bool, rd int32, rs1 int32, rs2 int32) uint32 {
	var funct3 int32 = 2
	var funct7 int32 = 2<<2 | boolInt(aq)<<1 | boolInt(rl)
	return Instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

func Rv32a_scw(aq bool, rl bool, rd int32, rs1 int32, rs2 int32) uint32 {
	var funct3 int32 = 2
	var funct7 int32 = 3<<2 | boolInt(aq)<<1 | boolInt(rl)
	return Instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

func Rv32a_amoswapw(aq bool, rl bool, rd int32, rs1 int32, rs2 int32) uint32 {
	var funct3 int32 = 2
	var funct7 int32 = 1<<2 | boolInt(aq)<<1 | boolInt(rl)
	return Instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

func Rv32a_amoaddw(aq bool, rl bool, rd int32, rs1 int32, rs2 int32) uint32 {
	var funct3 int32 = 2
	var funct7 int32 = boolInt(aq)<<1 | boolInt(rl)
	return Instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

func Rv32a_amoandw(aq bool, rl bool, rd int32, rs1 int32, rs2 int32) uint32 {
	var funct3 int32 = 2
	var funct7 int32 = 12<<2 | boolInt(aq)<<1 | boolInt(rl)
	return Instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

func Rv32a_amoorw(aq bool, rl bool, rd int32, rs1 int32, rs2 int32) uint32 {
	var funct3 int32 = 2
	var funct7 int32 = 8<<2 | boolInt(aq)<<1 | boolInt(rl)
	return Instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

func Rv32a_amoxorw(aq bool, rl bool, rd int32, rs1 int32, rs2 int32) uint32 {
	var funct3 int32 = 2
	var funct7 int32 = 4<<2 | boolInt(aq)<<1 | boolInt(rl)
	return Instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

func Rv32a_amomaxw(aq bool, rl bool, rd int32, rs1 int32, rs2 int32) uint32 {
	var funct3 int32 = 2
	var funct7 int32 = 20<<2 | boolInt(aq)<<1 | boolInt(rl)
	return Instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

func Rv32a_amomaxuw(aq bool, rl bool, rd int32, rs1 int32, rs2 int32) uint32 {
	var funct3 int32 = 2
	var funct7 int32 = 28<<2 | boolInt(aq)<<1 | boolInt(rl)
	return Instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

func Rv32a_amominw(aq bool, rl bool, rd int32, rs1 int32, rs2 int32) uint32 {
	var funct3 int32 = 2
	var funct7 int32 = 16<<2 | boolInt(aq)<<1 | boolInt(rl)
	return Instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}

func Rv32a_amominuw(aq bool, rl bool, rd int32, rs1 int32, rs2 int32) uint32 {
	var funct3 int32 = 2
	var funct7 int32 = 24<<2 | boolInt(aq)<<1 | boolInt(rl)
	return Instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
}
