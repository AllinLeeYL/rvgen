// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

package riscv

// Bit encoders preserve the original argument order and immediate masking.
const ZICSR_OPCODE_CSR int32 = 115

func Zicsr_csrrw(rd int32, rs1 int32, csr int32) uint32 {
	return Instruc_itype(ZICSR_OPCODE_CSR, rd, 1, rs1, csr)
}

func Zicsr_csrrs(rd int32, rs1 int32, csr int32) uint32 {
	return Instruc_itype(ZICSR_OPCODE_CSR, rd, 2, rs1, csr)
}

func Zicsr_csrrc(rd int32, rs1 int32, csr int32) uint32 {
	return Instruc_itype(ZICSR_OPCODE_CSR, rd, 3, rs1, csr)
}

func Zicsr_csrrwi(rd int32, uimm int32, csr int32) uint32 {
	return Instruc_itype(ZICSR_OPCODE_CSR, rd, 5, uimm, csr)
}

func Zicsr_csrrsi(rd int32, uimm int32, csr int32) uint32 {
	return Instruc_itype(ZICSR_OPCODE_CSR, rd, 6, uimm, csr)
}

func Zicsr_csrrci(rd int32, uimm int32, csr int32) uint32 {
	return Instruc_itype(ZICSR_OPCODE_CSR, rd, 7, uimm, csr)
}
