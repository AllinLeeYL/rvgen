// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

package riscv

// Bit encoders preserve the original argument order and immediate masking.
const RV32I_OPCODE_PRIVILEGED int32 = 115

func Rvprivileged_sret() uint32 {
	return Instruc_rtype(RV32I_OPCODE_PRIVILEGED, 0, 0, 0, 2, 8)
}

func Rvprivileged_mret() uint32 {
	return Instruc_rtype(RV32I_OPCODE_PRIVILEGED, 0, 0, 0, 2, 24)
}

func Rvprivileged_wfi() uint32 {
	return Instruc_rtype(RV32I_OPCODE_PRIVILEGED, 0, 0, 0, 5, 8)
}

func Rvprivileged_sfence_vma(rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32I_OPCODE_PRIVILEGED, 0, 0, rs1, rs2, 9)
}

func Rvprivileged_sinval_vma(rs1 int32, rs2 int32) uint32 {
	return Instruc_rtype(RV32I_OPCODE_PRIVILEGED, 0, 0, rs1, rs2, 11)
}

func Rvprivileged_sfence_w_inval() uint32 {
	return Instruc_rtype(RV32I_OPCODE_PRIVILEGED, 0, 0, 0, 0, 12)
}

func Rvprivileged_sfence_inval_ir() uint32 {
	return Instruc_rtype(RV32I_OPCODE_PRIVILEGED, 0, 0, 0, 1, 12)
}
