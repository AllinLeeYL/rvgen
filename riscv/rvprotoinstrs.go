// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

package riscv

// Bit encoders preserve the original argument order and immediate masking.

func Instruc_rtype(opcode int32, rd int32, funct3 int32, rs1 int32, rs2 int32, funct7 int32) uint32 {
	assert(opcode >= 0)
	assert(rd >= 0)
	assert(funct3 >= 0)
	assert(rs1 >= 0)
	assert(rs2 >= 0)
	assert(funct7 >= 0)
	assert(opcode < 1<<7)
	assert(rd < 32)
	assert(funct3 < 8)
	assert(rs1 < 32)
	assert(rs2 < 32)
	assert(funct7 < 1<<7)
	var rd_offset int32 = 7
	var funct3_offset int32 = rd_offset + 5
	var rs1_offset int32 = funct3_offset + 3
	var rs2_offset int32 = rs1_offset + 5
	var funct7_offset int32 = rs2_offset + 5
	return uint32(opcode | rd<<rd_offset | funct3<<funct3_offset | rs1<<rs1_offset | rs2<<rs2_offset | funct7<<funct7_offset)
}

func Instruc_itype(opcode int32, rd int32, funct3 int32, rs1 int32, imm int32) uint32 {
	assert(opcode < 1<<7)
	assert(rd < 32)
	assert(funct3 < 8)
	assert(rs1 < 32)
	assert(opcode >= 0)
	assert(rd >= 0)
	assert(funct3 >= 0)
	assert(rs1 >= 0)
	imm = imm & 4095
	var rd_offset int32 = 7
	var funct3_offset int32 = rd_offset + 5
	var rs1_offset int32 = funct3_offset + 3
	var imm_offset int32 = rs1_offset + 5
	return uint32(opcode | rd<<rd_offset | funct3<<funct3_offset | rs1<<rs1_offset | imm<<imm_offset)
}

func Instruc_stype(opcode int32, funct3 int32, rs1 int32, rs2 int32, imm int32) uint32 {
	assert(opcode < 1<<7)
	assert(funct3 < 8)
	assert(rs1 < 32)
	assert(rs2 < 32)
	assert(opcode >= 0)
	assert(funct3 >= 0)
	assert(rs1 >= 0)
	assert(rs2 >= 0)
	imm = imm & 4095
	var imm4_0_offset int32 = 7
	var funct3_offset int32 = imm4_0_offset + 5
	var rs1_offset int32 = funct3_offset + 3
	var rs2_offset int32 = rs1_offset + 5
	var imm11_5_offset int32 = rs2_offset + 5
	var imm4_0 int32 = imm & 31
	var imm11_5 int32 = imm >> 5 & 127
	return uint32(opcode | imm4_0<<imm4_0_offset | funct3<<funct3_offset | rs1<<rs1_offset | rs2<<rs2_offset | imm11_5<<imm11_5_offset)
}

func Instruc_btype(opcode int32, funct3 int32, rs1 int32, rs2 int32, imm int32) uint32 {
	assert(opcode < 1<<7)
	assert(funct3 < 8)
	assert(rs1 < 32)
	assert(rs2 < 32)
	assert(opcode >= 0)
	assert(funct3 >= 0)
	assert(rs1 >= 0)
	assert(rs2 >= 0)
	imm = imm & 8191
	var imm4_1_11_offset int32 = 7
	var funct3_offset int32 = imm4_1_11_offset + 5
	var rs1_offset int32 = funct3_offset + 3
	var rs2_offset int32 = rs1_offset + 5
	var imm12_10_5_offset int32 = rs2_offset + 5
	var imm4_1_11 int32 = imm>>11&1 | imm>>1&15<<1
	var imm12_10_5 int32 = imm>>5&63 | imm>>12&1<<6
	return uint32(opcode | imm4_1_11<<imm4_1_11_offset | funct3<<funct3_offset | rs1<<rs1_offset | rs2<<rs2_offset | imm12_10_5<<imm12_10_5_offset)
}

func Instruc_utype(opcode int32, rd int32, imm int32) uint32 {
	assert(opcode < 1<<7)
	assert(rd < 32)
	assert(opcode >= 0)
	assert(rd >= 0)
	var rd_offset int32 = 7
	var imm31_12_offset int32 = rd_offset + 5
	var imm31_12 int32 = imm & 1048575
	return uint32(opcode | rd<<rd_offset | imm31_12<<imm31_12_offset)
}

func Instruc_jtype(opcode int32, rd int32, imm int32) uint32 {
	assert(opcode < 1<<7)
	assert(rd < 32)
	assert(opcode >= 0)
	assert(rd >= 0)
	var rd_offset int32 = 7
	var immparts_offset int32 = rd_offset + 5
	var immparts int32 = imm>>12&255 | imm>>11&1<<8 | imm>>1&1023<<9 | imm>>20&1<<19
	return uint32(opcode | rd<<rd_offset | immparts<<immparts_offset)
}

func Instruc_r4type(opcode int32, rd int32, funct3 int32, rs1 int32, rs2 int32, rs3 int32, funct2 int32) uint32 {
	assert(opcode < 1<<7)
	assert(rd < 32)
	assert(funct3 < 8)
	assert(rs1 < 32)
	assert(rs2 < 32)
	assert(rs3 < 32)
	assert(funct2 < 1<<2)
	assert(opcode >= 0)
	assert(rd >= 0)
	assert(funct3 >= 0)
	assert(rs1 >= 0)
	assert(rs2 >= 0)
	assert(rs3 >= 0)
	assert(funct2 >= 0)
	var rd_offset int32 = 7
	var funct3_offset int32 = rd_offset + 5
	var rs1_offset int32 = funct3_offset + 3
	var rs2_offset int32 = rs1_offset + 5
	var funct2_offset int32 = rs2_offset + 5
	var rs3_offset int32 = funct2_offset + 2
	return uint32(opcode | rd<<rd_offset | funct3<<funct3_offset | rs1<<rs1_offset | rs2<<rs2_offset | funct2<<funct2_offset | rs3<<rs3_offset)
}

func Instruc_crtype(opcode int32, rs2 int32, rds1 int32, funct4 int32) uint32 {
	assert(opcode < 4)
	assert(rs2 < 32)
	assert(rds1 < 32)
	assert(funct4 < 1<<4)
	assert(opcode >= 0)
	assert(rs2 >= 0)
	assert(rds1 >= 0)
	assert(funct4 >= 0)
	var rs2_offset int32 = 2
	var rds1_offset int32 = rs2_offset + 5
	var funct4_offset int32 = rds1_offset + 5
	return uint32(opcode | rs2<<rs2_offset | rds1<<rds1_offset | funct4<<funct4_offset)
}

func Instruc_citype(opcode int32, rds1 int32, funct3 int32, imm int32) uint32 {
	assert(opcode < 4)
	assert(rds1 < 32)
	assert(funct3 < 1<<3)
	var imm4_0_offset int32 = 2
	var rds1_offset int32 = imm4_0_offset + 5
	var imm5_offset int32 = rds1_offset + 5
	var funct3_offset int32 = imm5_offset + 1
	var imm4_0 int32 = imm & 31
	var imm5 int32 = imm >> 5 & 1
	return uint32(opcode | imm4_0<<imm4_0_offset | rds1<<rds1_offset | imm5<<imm5_offset | funct3<<funct3_offset)
}

func Instruc_csstype(opcode int32, rs2 int32, funct3 int32, imm int32) uint32 {
	assert(opcode < 4)
	assert(rs2 < 32)
	assert(funct3 < 1<<3)
	assert(imm < 1<<7)
	var rs2_offset int32 = 2
	var imm_offset int32 = rs2_offset + 5
	var funct3_offset int32 = imm_offset + 6
	return uint32(opcode | rs2<<rs2_offset | imm<<imm_offset | funct3<<funct3_offset)
}

func Instruc_ciwtype(opcode int32, rdprime int32, funct3 int32, imm int32) uint32 {
	assert(opcode < 4)
	assert(rdprime < 8)
	assert(funct3 < 1<<3)
	assert(imm < 1<<9)
	var rdprime_offset int32 = 2
	var imm_offset int32 = rdprime_offset + 3
	var funct3_offset int32 = imm_offset + 9
	return uint32(opcode | rdprime<<rdprime_offset | imm<<imm_offset | funct3<<funct3_offset)
}

func Instruc_cltype(opcode int32, rdprime int32, rs1prime int32, funct3 int32, imm int32) uint32 {
	assert(opcode < 4)
	assert(rdprime < 8)
	assert(rs1prime < 8)
	assert(funct3 < 1<<3)
	assert(imm < 1<<5)
	var rdprime_offset int32 = 2
	var imm1_0_offset int32 = rdprime_offset + 3
	var rs1prime_offset int32 = imm1_0_offset + 2
	var imm4_2_offset int32 = rs1prime_offset + 3
	var funct3_offset int32 = imm4_2_offset + 3
	var imm1_0 int32 = imm & 3
	var imm4_2 int32 = imm >> 2 & 7
	return uint32(opcode | rdprime<<rdprime_offset | imm1_0<<imm1_0_offset | rs1prime<<rs1prime_offset | imm4_2<<imm4_2_offset | funct3<<funct3_offset)
}

func Instruc_cstype(opcode int32, rs1prime int32, rs2prime int32, funct3 int32, imm int32) uint32 {
	assert(opcode < 4)
	assert(rs1prime < 8)
	assert(rs2prime < 8)
	assert(funct3 < 1<<3)
	assert(imm < 1<<5)
	var rs2prime_offset int32 = 2
	var imm1_0_offset int32 = rs2prime_offset + 3
	var rs1prime_offset int32 = imm1_0_offset + 2
	var imm4_2_offset int32 = rs1prime_offset + 3
	var funct3_offset int32 = imm4_2_offset + 3
	var imm1_0 int32 = imm & 3
	var imm4_2 int32 = imm >> 2 & 7
	return uint32(opcode | rs2prime<<rs2prime_offset | imm1_0<<imm1_0_offset | rs1prime<<rs1prime_offset | imm4_2<<imm4_2_offset | funct3<<funct3_offset)
}

func Instruc_catype(opcode int32, rs2prime int32, funct2 int32, rds1prime int32, funct6 int32) uint32 {
	assert(opcode < 4)
	assert(rds1prime < 8)
	assert(rs2prime < 8)
	assert(funct2 < 4)
	assert(funct6 < 1<<6)
	var rs2prime_offset int32 = 2
	var funct2_offset int32 = rs2prime_offset + 3
	var rds1prime_offset int32 = funct2_offset + 2
	var funct6_offset int32 = rds1prime_offset + 3
	return uint32(opcode | rs2prime<<rs2prime_offset | funct2<<funct2_offset | rds1prime<<rds1prime_offset | funct6<<funct6_offset)
}

func Instruc_cbtype(opcode int32, rs1prime int32, funct3 int32, imm int32) uint32 {
	assert(opcode < 4)
	assert(rs1prime < 8)
	assert(funct3 < 8)
	var imm4_0_offset int32 = 2
	var rs1prime_offset int32 = imm4_0_offset + 5
	var imm7_5_offset int32 = rs1prime_offset + 3
	var funct3_offset int32 = imm7_5_offset + 3
	var imm4_0 int32 = imm & 31
	var imm7_5 int32 = imm >> 5 & 7
	return uint32(opcode | imm4_0<<imm4_0_offset | rs1prime<<rs1prime_offset | imm7_5<<imm7_5_offset | funct3<<funct3_offset)
}

func Instruc_cjtype(opcode int32, funct3 int32, imm int32) uint32 {
	assert(opcode < 4)
	assert(funct3 < 8)
	assert(imm < 1<<11)
	var imm_offset int32 = 2
	var funct3_offset int32 = imm_offset + 11
	return uint32(opcode | imm<<imm_offset | funct3<<funct3_offset)
}
