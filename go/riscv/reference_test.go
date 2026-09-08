package riscv

import "testing"

// Reference words verified with GNU RISC-V tools, from tests/instruction.rs.
func TestGNUReferenceEncodings(t *testing.T) {
	cases := []struct {
		i    Instruction
		want uint32
	}{
		{Instruction{Kind: Addi, Rd: Ra, Rs1: Zero, Imm: -1}, 0xfff00093},
		{Instruction{Kind: Add, Rd: T0, Rs1: T1, Rs2: T2}, 0x007302b3},
		{Instruction{Kind: Sub, Rd: T0, Rs1: T1, Rs2: T2}, 0x407302b3},
		{Instruction{Kind: Sw, Rs1: T1, Rs2: T2, Imm: -16}, 0xfe732823},
		{Instruction{Kind: Beq, Rs1: Ra, Rs2: Sp, Imm: -4}, 0xfe208ee3},
		{Instruction{Kind: Lui, Rd: T0, Imm: 0x12345}, 0x123452b7},
		{Instruction{Kind: Jal, Rd: Ra, Imm: 8}, 0x008000ef},
		{Instruction{Kind: Ld, Rd: T0, Rs1: T1, Imm: 24}, 0x01833283},
		{Instruction{Kind: Addw, Rd: T0, Rs1: T1, Rs2: T2}, 0x007302bb},
		{Instruction{Kind: Mul, Rd: T0, Rs1: T1, Rs2: T2}, 0x027302b3},
		{Instruction{Kind: Mulw, Rd: T0, Rs1: T1, Rs2: T2}, 0x027302bb},
		{Instruction{Kind: LrW, Rd: T0, Rs1: T1, Aq: true, Rl: false}, 0x140322af},
		{Instruction{Kind: ScD, Rd: T0, Rs1: T1, Rs2: T2, Aq: false, Rl: true}, 0x1a7332af},
		{Instruction{Kind: AmoaddD, Rd: T0, Rs1: T1, Rs2: T2, Aq: true, Rl: true}, 0x067332af},
		{Instruction{Kind: Flw, Rd: Ft5, Rs1: T1, Imm: 12}, 0x00c32287},
		{Instruction{Kind: Fsd, Rs1: T1, Rs2: Ft7, Imm: 16}, 0x00733827},
		{Instruction{Kind: FmaddS, Rd: Ft5, Rs1: Ft6, Rs2: Ft7, Rs3: Fs0, Rm: 0}, 0x407302c3},
		{Instruction{Kind: FaddD, Rd: Ft5, Rs1: Ft6, Rs2: Ft7, Rm: 1}, 0x027312d3},
		{Instruction{Kind: FcvtLS, Rd: T0, Rs1: Ft6, Rm: 0}, 0xc02302d3},
		{Instruction{Kind: FcvtDL, Rd: Ft5, Rs1: T1, Rm: 0}, 0xd22302d3},
		{Instruction{Kind: FmvXD, Rd: T0, Rs1: Ft6}, 0xe20302d3},
		{Instruction{Kind: Csrrw, Rd: T0, Rs1: T1, Csr: 0x300}, 0x300312f3},
		{Instruction{Kind: Csrrwi, Rd: T0, Uimm: 7, Csr: 0x300}, 0x3003d2f3},
		{Instruction{Kind: Fence, Imm: 0x33}, 0x0330000f},
		{Instruction{Kind: FenceI, Imm: 0}, 0x0000100f},
		{Instruction{Kind: CAddi, Rd: Ra, Imm: 1}, 0x0085},
		{Instruction{Kind: CAddiw, Rd: Ra, Imm: 1}, 0x2085},
		{Instruction{Kind: CMv, Rd: T0, Rs2: T1}, 0x829a},
	}
	for _, c := range cases {
		if got := c.i.Encode().Bits; got != c.want {
			t.Errorf("%s: got %08x want %08x", c.i.Mnemonic(), got, c.want)
		}
	}
}
