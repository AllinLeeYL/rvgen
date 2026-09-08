// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

package riscv

// Bit encoders preserve the original argument order and immediate masking.
const RV64D_OPCODE_FCVT int32 = 83

func Rv64d_fcvtld(rd int32, rs1 int32, rm int32) uint32 {
	return Instruc_rtype(RV64D_OPCODE_FCVT, rd, rm, rs1, 2, 97)
}

func Rv64d_fcvtlud(rd int32, rs1 int32, rm int32) uint32 {
	return Instruc_rtype(RV64D_OPCODE_FCVT, rd, rm, rs1, 3, 97)
}

func Rv64d_fmvxd(rd int32, rs1 int32) uint32 {
	return Instruc_rtype(RV64D_OPCODE_FCVT, rd, 0, rs1, 0, 113)
}

func Rv64d_fcvtdl(rd int32, rs1 int32, rm int32) uint32 {
	return Instruc_rtype(RV64D_OPCODE_FCVT, rd, rm, rs1, 2, 105)
}

func Rv64d_fcvtdlu(rd int32, rs1 int32, rm int32) uint32 {
	return Instruc_rtype(RV64D_OPCODE_FCVT, rd, rm, rs1, 3, 105)
}

func Rv64d_fmvdx(rd int32, rs1 int32) uint32 {
	return Instruc_rtype(RV64D_OPCODE_FCVT, rd, 0, rs1, 0, 121)
}
