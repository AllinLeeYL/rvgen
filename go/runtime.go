package rvgen

import (
	"encoding/binary"
	"fmt"
	"rvgen/riscv"
)

func pcRelative(delta int64) ([2]uint32, error) {
	hi := (delta + 0x800) >> 12
	if hi < -0x80000 || hi > 0x7ffff {
		return [2]uint32{}, fmt.Errorf("runtime target exceeds AUIPC range")
	}
	return [2]uint32{riscv.Rv32i_auipc(5, int32(hi)), riscv.Rv32i_addi(5, 5, int32(delta-(hi<<12)))}, nil
}
func appendWords(dst []byte, words ...uint32) []byte {
	for _, w := range words {
		dst = binary.LittleEndian.AppendUint32(dst, w)
	}
	return dst
}

// Executable wraps a workload in a machine-mode trap handler and HTIF exit
// runtime. Normal completion writes 1 to tohost; traps write 3 (failure).
func Executable(body []byte, is64 bool, start uint64) ([]byte, error) {
	if start&3 != 0 || len(body)&1 != 0 {
		return nil, fmt.Errorf("entry must be four-byte aligned and body must contain whole instructions")
	}
	text := make([]byte, 8, len(body)+64)
	text = appendWords(text, riscv.Zicsr_csrrw(0, 5, riscv.CSR_MTVEC))
	text = append(text, body...)
	if len(text)&3 != 0 {
		text = append(text, 1, 0)
	}
	text = appendWords(text, riscv.Rv32i_addi(31, 0, 1))
	writerOffset := len(text)
	text = append(text, make([]byte, 8)...)
	store := riscv.Rv64i_sd(5, 31, 0)
	if !is64 {
		text = appendWords(text, riscv.Rv32i_sw(5, 0, 4))
		store = riscv.Rv32i_sw(5, 31, 0)
	}
	text = appendWords(text, riscv.Rv32i_fence(0x33), store, riscv.Rv32i_fence(0x33), riscv.Rv32i_jal(0, 0))
	trapOffset := len(text)
	text = appendWords(text, riscv.Rv32i_addi(31, 0, 3), riscv.Rv32i_jal(0, int32(writerOffset-trapOffset-4)))
	textEnd, err := add64(start, uint64(len(text)))
	if err != nil {
		return nil, err
	}
	tohostAddr, err := alignUp(textEnd, 64)
	if err != nil {
		return nil, err
	}
	fromhostAddr, err := add64(tohostAddr, 64)
	if err != nil {
		return nil, err
	}
	entryPair, err := pcRelative(int64(trapOffset))
	if err != nil {
		return nil, err
	}
	writerPair, err := pcRelative(int64(tohostAddr-start) - int64(writerOffset))
	if err != nil {
		return nil, err
	}
	for i, w := range entryPair {
		binary.LittleEndian.PutUint32(text[i*4:], w)
	}
	for i, w := range writerPair {
		binary.LittleEndian.PutUint32(text[writerOffset+i*4:], w)
	}
	code := NewELFSection(".text", text)
	code.Addr = start
	tohost := ELFSection{Name: ".tohost", Bytes: make([]byte, 8), Addr: tohostAddr, Flags: SHFAlloc | SHFWrite, Align: 64, Type: 1}
	fromhost := tohost
	fromhost.Name = ".fromhost"
	fromhost.Addr = fromhostAddr
	symbols := []ELFSymbol{
		{"_start", 0, 0, uint64(len(text)), 2}, {"rvgen_trap", 0, uint64(trapOffset), 8, 2},
		{"tohost", 1, 0, 8, 1}, {"fromhost", 2, 0, 8, 1},
	}
	return BuildELFWithSymbols([]ELFSection{code, tohost, fromhost}, symbols, is64, start)
}
