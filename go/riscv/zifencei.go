// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

package riscv

// Bit encoders preserve the original argument order and immediate masking.
const ZIFENCEI_OPCODE_FENCEI int32 = 15

func Zifencei_fencei(imm int32) uint32 {
	return Instruc_itype(ZIFENCEI_OPCODE_FENCEI, 0, 1, 0, imm)
}
