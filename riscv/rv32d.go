// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

package riscv

// Bit encoders preserve the original argument order and immediate masking.
const RV32D_OPCODE_FLD int32 = 7
const RV32D_OPCODE_FSD int32 = 39
const RV32D_OPCODE_FMADDD int32 = 67
const RV32D_OPCODE_FMSUBD int32 = 71
const RV32D_OPCODE_FNMSUBD int32 = 75
const RV32D_OPCODE_FNMADDD int32 = 79
const RV32D_OPCODE_FALU int32 = 83

func Rv32d_fld(rd int32, rs1 int32, imm int32) uint32 {
	return Instruc_itype(RV32D_OPCODE_FLD, rd, 3, rs1, imm)
}

func Rv32d_fsd(rs1 int32, rs2 int32, imm int32) uint32 {
	return Instruc_stype(RV32D_OPCODE_FSD, 3, rs1, rs2, imm)
}

func Rv32d_fmaddd(rd int32, rs1 int32, rs2 int32, rs3 int32, rm int32) uint32 {
	return Instruc_r4type(RV32D_OPCODE_FMADDD, rd, rm, rs1, rs2, rs3, 1)
}

func Rv32d_fmsubd(rd int32, rs1 int32, rs2 int32, rs3 int32, rm int32) uint32 {
	return Instruc_r4type(RV32D_OPCODE_FMSUBD, rd, rm, rs1, rs2, rs3, 1)
}

func Rv32d_fnmsubd(rd int32, rs1 int32, rs2 int32, rs3 int32, rm int32) uint32 {
	return Instruc_r4type(RV32D_OPCODE_FNMSUBD, rd, rm, rs1, rs2, rs3, 1)
}

func Rv32d_fnmaddd(rd int32, rs1 int32, rs2 int32, rs3 int32, rm int32) uint32 {
	return Instruc_r4type(RV32D_OPCODE_FNMADDD, rd, rm, rs1, rs2, rs3, 1)
}

func Rv32d_faddd(rd int32, rs1 int32, rs2 int32, rm int32) uint32 {
	return Instruc_rtype(RV32D_OPCODE_FALU, rd, rm, rs1, rs2, 1)
}

func Rv32d_fsubd(rd int32, rs1 int32, rs2 int32, rm int32) uint32 {
	return Instruc_rtype(RV32D_OPCODE_FALU, rd, rm, rs1, rs2, 5)
}

func Rv32d_fmuld(rd int32, rs1 int32, rs2 int32, rm int32) uint32 {
	return Instruc_rtype(RV32D_OPCODE_FALU, rd, rm, rs1, rs2, 9)
}

func Rv32d_fdivd(rd int32, rs1 int32, rs2 int32, rm int32) uint32 {
	return Instruc_rtype(RV32D_OPCODE_FALU, rd, rm, rs1, rs2, 13)
}

func Rv32d_fsqrtd(rd int32, rs1 int32, rm int32) uint32 {
	return Instruc_rtype(RV32D_OPCODE_FALU, rd, rm, rs1, 0, 45)
}

func Rv32d_fsgnjd(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32D_OPCODE_FALU, rd, 0, rs1, rs2, 17)
}

func Rv32d_fsgnjnd(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32D_OPCODE_FALU, rd, 1, rs1, rs2, 17)
}

func Rv32d_fsgnjxd(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32D_OPCODE_FALU, rd, 2, rs1, rs2, 17)
}

func Rv32d_fmind(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32D_OPCODE_FALU, rd, 0, rs1, rs2, 21)
}

func Rv32d_fmaxd(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32D_OPCODE_FALU, rd, 1, rs1, rs2, 21)
}

func Rv32d_fcvtsd(rd int32, rs1 int32, rm int32) uint32 {
	return Instruc_rtype(RV32D_OPCODE_FALU, rd, rm, rs1, 1, 32)
}

func Rv32d_fcvtds(rd int32, rs1 int32, rm int32) uint32 {
	return Instruc_rtype(RV32D_OPCODE_FALU, rd, rm, rs1, 0, 33)
}

func Rv32d_feqd(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32D_OPCODE_FALU, rd, 2, rs1, rs2, 81)
}

func Rv32d_fltd(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32D_OPCODE_FALU, rd, 1, rs1, rs2, 81)
}

func Rv32d_fled(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32D_OPCODE_FALU, rd, 0, rs1, rs2, 81)
}

func Rv32d_fclassd(rd int32, rs1 int32) uint32 {
	return Instruc_rtype(RV32D_OPCODE_FALU, rd, 1, rs1, 0, 113)
}

func Rv32d_fcvtwd(rd int32, rs1 int32, rm int32) uint32 {
	return Instruc_rtype(RV32D_OPCODE_FALU, rd, rm, rs1, 0, 97)
}

func Rv32d_fcvtwud(rd int32, rs1 int32, rm int32) uint32 {
	return Instruc_rtype(RV32D_OPCODE_FALU, rd, rm, rs1, 1, 97)
}

func Rv32d_fcvtdw(rd int32, rs1 int32, rm int32) uint32 {
	return Instruc_rtype(RV32D_OPCODE_FALU, rd, rm, rs1, 0, 105)
}

func Rv32d_fcvtdwu(rd int32, rs1 int32, rm int32) uint32 {
	return Instruc_rtype(RV32D_OPCODE_FALU, rd, rm, rs1, 1, 105)
}
