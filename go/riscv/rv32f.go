// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

package riscv

// Bit encoders preserve the original argument order and immediate masking.
const RV32F_OPCODE_FLW int32 = 7
const RV32F_OPCODE_FSW int32 = 39
const RV32F_OPCODE_FMADDS int32 = 67
const RV32F_OPCODE_FMSUBS int32 = 71
const RV32F_OPCODE_FNMSUBS int32 = 75
const RV32F_OPCODE_FNMADDS int32 = 79
const RV32F_OPCODE_FALU int32 = 83

func Rv32f_flw(rd int32, rs1 int32, imm int32) uint32 {
	return Instruc_itype(RV32F_OPCODE_FLW, rd, 2, rs1, imm)
}

func Rv32f_fsw(rs1 int32, rs2 int32, imm int32) uint32 {
	return Instruc_stype(RV32F_OPCODE_FSW, 2, rs1, rs2, imm)
}

func Rv32f_fmadds(rd int32, rs1 int32, rs2 int32, rs3 int32, rm int32) uint32 {
	return Instruc_r4type(RV32F_OPCODE_FMADDS, rd, rm, rs1, rs2, rs3, 0)
}

func Rv32f_fmsubs(rd int32, rs1 int32, rs2 int32, rs3 int32, rm int32) uint32 {
	return Instruc_r4type(RV32F_OPCODE_FMSUBS, rd, rm, rs1, rs2, rs3, 0)
}

func Rv32f_fnmsubs(rd int32, rs1 int32, rs2 int32, rs3 int32, rm int32) uint32 {
	return Instruc_r4type(RV32F_OPCODE_FNMSUBS, rd, rm, rs1, rs2, rs3, 0)
}

func Rv32f_fnmadds(rd int32, rs1 int32, rs2 int32, rs3 int32, rm int32) uint32 {
	return Instruc_r4type(RV32F_OPCODE_FNMADDS, rd, rm, rs1, rs2, rs3, 0)
}

func Rv32f_fadds(rd int32, rs1 int32, rs2 int32, rm int32) uint32 {
	return Instruc_rtype(RV32F_OPCODE_FALU, rd, rm, rs1, rs2, 0)
}

func Rv32f_fsubs(rd int32, rs1 int32, rs2 int32, rm int32) uint32 {
	return Instruc_rtype(RV32F_OPCODE_FALU, rd, rm, rs1, rs2, 4)
}

func Rv32f_fmuls(rd int32, rs1 int32, rs2 int32, rm int32) uint32 {
	return Instruc_rtype(RV32F_OPCODE_FALU, rd, rm, rs1, rs2, 8)
}

func Rv32f_fdivs(rd int32, rs1 int32, rs2 int32, rm int32) uint32 {
	return Instruc_rtype(RV32F_OPCODE_FALU, rd, rm, rs1, rs2, 12)
}

func Rv32f_fsqrts(rd int32, rs1 int32, rm int32) uint32 {
	return Instruc_rtype(RV32F_OPCODE_FALU, rd, rm, rs1, 0, 44)
}

func Rv32f_fsgnjs(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32F_OPCODE_FALU, rd, 0, rs1, rs2, 16)
}

func Rv32f_fsgnjns(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32F_OPCODE_FALU, rd, 1, rs1, rs2, 16)
}

func Rv32f_fsgnjxs(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32F_OPCODE_FALU, rd, 2, rs1, rs2, 16)
}

func Rv32f_fmins(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32F_OPCODE_FALU, rd, 0, rs1, rs2, 20)
}

func Rv32f_fmaxs(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32F_OPCODE_FALU, rd, 1, rs1, rs2, 20)
}

func Rv32f_fcvtws(rd int32, rs1 int32, rm int32) uint32 {
	return Instruc_rtype(RV32F_OPCODE_FALU, rd, rm, rs1, 0, 96)
}

func Rv32f_fcvtwus(rd int32, rs1 int32, rm int32) uint32 {
	return Instruc_rtype(RV32F_OPCODE_FALU, rd, rm, rs1, 1, 96)
}

func Rv32f_fmvxw(rd int32, rs1 int32) uint32 {
	return Instruc_rtype(RV32F_OPCODE_FALU, rd, 0, rs1, 0, 112)
}

func Rv32f_feqs(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32F_OPCODE_FALU, rd, 2, rs1, rs2, 80)
}

func Rv32f_flts(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32F_OPCODE_FALU, rd, 1, rs1, rs2, 80)
}

func Rv32f_fles(rd int32, rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32F_OPCODE_FALU, rd, 0, rs1, rs2, 80)
}

func Rv32f_fclasss(rd int32, rs1 int32) uint32 {
	return Instruc_rtype(RV32F_OPCODE_FALU, rd, 1, rs1, 0, 112)
}

func Rv32f_fcvtsw(rd int32, rs1 int32, rm int32) uint32 {
	return Instruc_rtype(RV32F_OPCODE_FALU, rd, rm, rs1, 0, 104)
}

func Rv32f_fcvtswu(rd int32, rs1 int32, rm int32) uint32 {
	return Instruc_rtype(RV32F_OPCODE_FALU, rd, rm, rs1, 1, 104)
}

func Rv32f_fmvwx(rd int32, rs1 int32) uint32 {
	return Instruc_rtype(RV32F_OPCODE_FALU, rd, 0, rs1, 0, 120)
}
