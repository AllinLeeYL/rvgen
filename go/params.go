// Package rvgen generates RISC-V instruction workloads and bare-metal ELF files.
package rvgen

import "fmt"

type GeneratorParams struct {
	Size, Memsize, NumCores, NumBBs int
	Seed                            int64
	AuthorizePrivileges, Is64Bit    bool
	StartAddr                       uint64
}

func DefaultParams() GeneratorParams {
	return GeneratorParams{Size: 256, Memsize: 4096, NumCores: 1, NumBBs: 12, AuthorizePrivileges: true, Is64Bit: true, StartAddr: 0x80000000}
}
func (p GeneratorParams) Validate() error {
	if p.Size < 0 || p.Memsize < 0 {
		return fmt.Errorf("size and memsize must be nonnegative")
	}
	if p.NumCores <= 0 {
		return fmt.Errorf("num_cores must be greater than zero")
	}
	if p.NumBBs <= 0 {
		return fmt.Errorf("num_bbs must be greater than zero")
	}
	if !p.Is64Bit && p.StartAddr > 0xffffffff {
		return fmt.Errorf("start_addr does not fit ELF32")
	}
	return nil
}
func partition(total, parts, index int) int {
	n := total / parts
	if index < total%parts {
		n++
	}
	return n
}
