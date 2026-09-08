// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only
package riscv

func LiIntoReg(value uint64, checkBounds bool) (uint32, int32) {
	if checkBounds && value >= 0x80000000 {
		panic("sign extension beyond 31 bits is not implemented")
	}
	carry := value >> 11 & 1
	return uint32((value>>12 + carry) & 0xfffff), int32(value&0xfff) - int32(carry)*0x1000
}
func TwosComplement(value uint64, is64 bool) int64 {
	if is64 {
		return int64(value)
	}
	assert(value <= 0xffffffff)
	return int64(int32(uint32(value)))
}
func ToUnsigned(value int64, is64 bool) uint64 {
	if is64 {
		return uint64(value)
	}
	assert(value >= -0x80000000 && value <= 0xffffffff)
	return uint64(uint32(value))
}
