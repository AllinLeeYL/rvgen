// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

package riscv

// Bit encoders preserve the original argument order and immediate masking.
const RV64M_OPCODE_MUL int32 = 59

func Rv64m_mulw(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV64M_OPCODE_MUL, rd, 0, rs1, rs2, 1)
}

func Rv64m_divw(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV64M_OPCODE_MUL, rd, 4, rs1, rs2, 1)
}

func Rv64m_divuw(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV64M_OPCODE_MUL, rd, 5, rs1, rs2, 1)
}

func Rv64m_remw(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV64M_OPCODE_MUL, rd, 6, rs1, rs2, 1)
}

func Rv64m_remuw(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV64M_OPCODE_MUL, rd, 7, rs1, rs2, 1)
}
