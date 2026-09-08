// Build an explicit workload without random generation.
package main

import (
	"fmt"
	"os"

	"rvgen"
	"rvgen/riscv"
)

func main() {
	p := rvgen.DefaultParams()
	p.NumBBs = 1
	g, err := rvgen.NewGenerator(p)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	g.Cores[0].Blocks[0].Instructions = []riscv.Instruction{
		{Kind: riscv.Addi, Rd: riscv.A0, Rs1: riscv.Zero, Imm: 42},
		{Kind: riscv.Add, Rd: riscv.A1, Rs1: riscv.A0, Rs2: riscv.A0},
	}
	output := "encoded.elf"
	if len(os.Args) > 1 {
		output = os.Args[1]
	}
	if err = g.GenELF(output); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
