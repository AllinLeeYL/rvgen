// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

package riscv

// Bit encoders preserve the original argument order and immediate masking.
const RV32M_OPCODE_MUL int32 = 51

func Rv32m_mul(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32M_OPCODE_MUL, rd, 0, rs1, rs2, 1)
}

func Rv32m_mulh(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32M_OPCODE_MUL, rd, 1, rs1, rs2, 1)
}

func Rv32m_mulhsu(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32M_OPCODE_MUL, rd, 2, rs1, rs2, 1)
}

func Rv32m_mulhu(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32M_OPCODE_MUL, rd, 3, rs1, rs2, 1)
}

func Rv32m_div(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32M_OPCODE_MUL, rd, 4, rs1, rs2, 1)
}

func Rv32m_divu(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32M_OPCODE_MUL, rd, 5, rs1, rs2, 1)
}

func Rv32m_rem(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32M_OPCODE_MUL, rd, 6, rs1, rs2, 1)
}

func Rv32m_remu(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32M_OPCODE_MUL, rd, 7, rs1, rs2, 1)
}
