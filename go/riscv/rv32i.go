// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

package riscv

// Bit encoders preserve the original argument order and immediate masking.
const RV32I_OPCODE_LUI int32 = 55
const RV32I_OPCODE_AUIPC int32 = 23
const RV32I_OPCODE_JAL int32 = 111
const RV32I_OPCODE_JALR int32 = 103
const RV32I_OPCODE_B int32 = 99
const RV32I_OPCODE_L int32 = 3
const RV32I_OPCODE_S int32 = 35
const RV32I_OPCODE_ALU_IMM int32 = 19
const RV32I_OPCODE_ALU_REG int32 = 51
const RV32I_OPCODE_FEN int32 = 15
const RV32I_OPCODE_E int32 = 115

func Rv32i_lui(rd int32, imm int32) uint32 {
	return Instruc_utype(RV32I_OPCODE_LUI, rd, imm)
}

func Rv32i_auipc(rd int32, imm int32) uint32 {
	return Instruc_utype(RV32I_OPCODE_AUIPC, rd, imm)
}

func Rv32i_jal(rd int32, imm int32) uint32 {
	return Instruc_jtype(RV32I_OPCODE_JAL, rd, imm)
}

func Rv32i_jalr(rd int32, rs1 int32, imm int32) uint32 {
	return Instruc_itype(RV32I_OPCODE_JALR, rd, 0, rs1, imm)
}

func Rv32i_beq(rs1 int32, rs2 int32, imm int32) uint32 {
	return Instruc_btype(RV32I_OPCODE_B, 0, rs1, rs2, imm)
}

func Rv32i_bne(rs1 int32, rs2 int32, imm int32) uint32 {
	return Instruc_btype(RV32I_OPCODE_B, 1, rs1, rs2, imm)
}

func Rv32i_blt(rs1 int32, rs2 int32, imm int32) uint32 {
	return Instruc_btype(RV32I_OPCODE_B, 4, rs1, rs2, imm)
}

func Rv32i_bge(rs1 int32, rs2 int32, imm int32) uint32 {
	return Instruc_btype(RV32I_OPCODE_B, 5, rs1, rs2, imm)
}

func Rv32i_bltu(rs1 int32, rs2 int32, imm int32) uint32 {
	return Instruc_btype(RV32I_OPCODE_B, 6, rs1, rs2, imm)
}

func Rv32i_bgeu(rs1 int32, rs2 int32, imm int32) uint32 {
	return Instruc_btype(RV32I_OPCODE_B, 7, rs1, rs2, imm)
}

func Rv32i_lb(rd int32, rs1 int32, imm int32) uint32 {
	return Instruc_itype(RV32I_OPCODE_L, rd, 0, rs1, imm)
}

func Rv32i_lh(rd int32, rs1 int32, imm int32) uint32 {
	return Instruc_itype(RV32I_OPCODE_L, rd, 1, rs1, imm)
}

func Rv32i_lw(rd int32, rs1 int32, imm int32) uint32 {
	return Instruc_itype(RV32I_OPCODE_L, rd, 2, rs1, imm)
}

func Rv32i_lbu(rd int32, rs1 int32, imm int32) uint32 {
	return Instruc_itype(RV32I_OPCODE_L, rd, 4, rs1, imm)
}

func Rv32i_lhu(rd int32, rs1 int32, imm int32) uint32 {
	return Instruc_itype(RV32I_OPCODE_L, rd, 5, rs1, imm)
}

func Rv32i_sb(rs1 int32, rs2 int32, imm int32) uint32 {
	return Instruc_stype(RV32I_OPCODE_S, 0, rs1, rs2, imm)
}

func Rv32i_sh(rs1 int32, rs2 int32, imm int32) uint32 {
	return Instruc_stype(RV32I_OPCODE_S, 1, rs1, rs2, imm)
}

func Rv32i_sw(rs1 int32, rs2 int32, imm int32) uint32 {
	return Instruc_stype(RV32I_OPCODE_S, 2, rs1, rs2, imm)
}

func Rv32i_addi(rd int32, rs1 int32, imm int32) uint32 {
	return Instruc_itype(RV32I_OPCODE_ALU_IMM, rd, 0, rs1, imm)
}

func Rv32i_slti(rd int32, rs1 int32, imm int32) uint32 {
	return Instruc_itype(RV32I_OPCODE_ALU_IMM, rd, 2, rs1, imm)
}

func Rv32i_sltiu(rd int32, rs1 int32, imm int32) uint32 {
	return Instruc_itype(RV32I_OPCODE_ALU_IMM, rd, 3, rs1, imm)
}

func Rv32i_xori(rd int32, rs1 int32, imm int32) uint32 {
	return Instruc_itype(RV32I_OPCODE_ALU_IMM, rd, 4, rs1, imm)
}

func Rv32i_ori(rd int32, rs1 int32, imm int32) uint32 {
	return Instruc_itype(RV32I_OPCODE_ALU_IMM, rd, 6, rs1, imm)
}

func Rv32i_andi(rd int32, rs1 int32, imm int32) uint32 {
	return Instruc_itype(RV32I_OPCODE_ALU_IMM, rd, 7, rs1, imm)
}

func Rv32i_slli(rd int32, rs1 int32, shamt int32) uint32 {
	var imm int32 = shamt
	return Instruc_itype(RV32I_OPCODE_ALU_IMM, rd, 1, rs1, imm)
}

func Rv32i_srli(rd int32, rs1 int32, shamt int32) uint32 {
	var imm int32 = shamt
	return Instruc_itype(RV32I_OPCODE_ALU_IMM, rd, 5, rs1, imm)
}

func Rv32i_srai(rd int32, rs1 int32, shamt int32) uint32 {
	var imm int32 = 1024 | shamt
	return Instruc_itype(RV32I_OPCODE_ALU_IMM, rd, 5, rs1, imm)
}

func Rv32i_add(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32I_OPCODE_ALU_REG, rd, 0, rs1, rs2, 0)
}

func Rv32i_sub(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32I_OPCODE_ALU_REG, rd, 0, rs1, rs2, 32)
}

func Rv32i_sll(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32I_OPCODE_ALU_REG, rd, 1, rs1, rs2, 0)
}

func Rv32i_slt(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32I_OPCODE_ALU_REG, rd, 2, rs1, rs2, 0)
}

func Rv32i_sltu(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32I_OPCODE_ALU_REG, rd, 3, rs1, rs2, 0)
}

func Rv32i_xor(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32I_OPCODE_ALU_REG, rd, 4, rs1, rs2, 0)
}

func Rv32i_srl(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32I_OPCODE_ALU_REG, rd, 5, rs1, rs2, 0)
}

func Rv32i_sra(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32I_OPCODE_ALU_REG, rd, 5, rs1, rs2, 32)
}

func Rv32i_or(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32I_OPCODE_ALU_REG, rd, 6, rs1, rs2, 0)
}

func Rv32i_and(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32I_OPCODE_ALU_REG, rd, 7, rs1, rs2, 0)
}

func Rv32i_fence(imm int32) uint32 {
	return Instruc_itype(RV32I_OPCODE_FEN, 0, 0, 0, imm)
}

func Rv32i_ecall() uint32 {
	return Instruc_itype(RV32I_OPCODE_E, 0, 0, 0, 0)
}

func Rv32i_ebreak() uint32 {
	return Instruc_itype(RV32I_OPCODE_E, 0, 0, 0, 1)
}
