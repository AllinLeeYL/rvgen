// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

package riscv

// Bit encoders preserve the original argument order and immediate masking.
const RV64I_OPCODE_LWU int32 = 3
const RV64I_OPCODE_LD int32 = 3
const RV64I_OPCODE_SD int32 = 35
const RV64I_OPCODE_ALU_IMM int32 = 27
const RV64I_OPCODE_ALU_REG int32 = 59

func Rv64i_lwu(rd int32, rs1 int32, imm int32) uint32 {
	return Instruc_itype(RV64I_OPCODE_LWU, rd, 6, rs1, imm)
}

func Rv64i_ld(rd int32, rs1 int32, imm int32) uint32 {
	return Instruc_itype(RV64I_OPCODE_LD, rd, 3, rs1, imm)
}

func Rv64i_sd(rs1 int32, rs2 int32, imm int32) uint32 {
	return Instruc_stype(RV64I_OPCODE_SD, 3, rs1, rs2, imm)
}

func Rv64i_addiw(rd int32, rs1 int32, imm int32) uint32 {
	return Instruc_itype(RV64I_OPCODE_ALU_IMM, rd, 0, rs1, imm)
}

func Rv64i_slliw(rd int32, rs1 int32, shamt int32) uint32 {
	var imm int32 = shamt
	return Instruc_itype(RV64I_OPCODE_ALU_IMM, rd, 1, rs1, imm)
}

func Rv64i_srliw(rd int32, rs1 int32, shamt int32) uint32 {
	var imm int32 = shamt
	return Instruc_itype(RV64I_OPCODE_ALU_IMM, rd, 5, rs1, imm)
}

func Rv64i_sraiw(rd int32, rs1 int32, shamt int32) uint32 {
	var imm int32 = 1024 | shamt
	return Instruc_itype(RV64I_OPCODE_ALU_IMM, rd, 5, rs1, imm)
}

func Rv64i_addw(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV64I_OPCODE_ALU_REG, rd, 0, rs1, rs2, 0)
}

func Rv64i_subw(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV64I_OPCODE_ALU_REG, rd, 0, rs1, rs2, 32)
}

func Rv64i_sllw(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV64I_OPCODE_ALU_REG, rd, 1, rs1, rs2, 0)
}

func Rv64i_srlw(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV64I_OPCODE_ALU_REG, rd, 5, rs1, rs2, 0)
}

func Rv64i_sraw(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV64I_OPCODE_ALU_REG, rd, 5, rs1, rs2, 32)
}
