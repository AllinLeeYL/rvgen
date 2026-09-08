// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

package riscv

// Bit encoders preserve the original argument order and immediate masking.
const RV64F_OPCODE_FCVT int32 = 83

func Rv64f_fcvtls(rd int32, rs1 int32, rm int32) uint32 {
	return Instruc_rtype(RV64F_OPCODE_FCVT, rd, rm, rs1, 2, 96)
}

func Rv64f_fcvtlus(rd int32, rs1 int32, rm int32) uint32 {
	return Instruc_rtype(RV64F_OPCODE_FCVT, rd, rm, rs1, 3, 96)
}

func Rv64f_fcvtsl(rd int32, rs1 int32, rm int32) uint32 {
	return Instruc_rtype(RV64F_OPCODE_FCVT, rd, rm, rs1, 2, 104)
}

func Rv64f_fcvtslu(rd int32, rs1 int32, rm int32) uint32 {
	return Instruc_rtype(RV64F_OPCODE_FCVT, rd, rm, rs1, 3, 104)
}
