package riscv

import (
	"encoding/json"
	"os"
	"reflect"
	"testing"
)

func TestRustEncodingParity(t *testing.T) {
	data, err := os.ReadFile("testdata/rust_encodings.json")
	if err != nil {
		t.Fatal(err)
	}
	var cases []struct {
		Instruction Instruction
		Name        string
		Width       int
		Bits        uint32
		Panics      bool
	}
	if err = json.Unmarshal(data, &cases); err != nil {
		t.Fatal(err)
	}
	seen := map[InstructionKind]bool{}
	for _, c := range cases {
		seen[c.Instruction.Kind] = true
		func() {
			defer func() {
				if r := recover(); r != nil && !c.Panics {
					t.Errorf("%s %+v unexpectedly panicked: %v", c.Name, c.Instruction, r)
				}
			}()
			got := c.Instruction.Encode()
			if c.Panics {
				t.Errorf("%s expected panic for %+v", c.Name, c.Instruction)
				return
			}
			if got.Bits != c.Bits || got.Width != c.Width {
				t.Errorf("%s %+v: got %+v want %08x/%d", c.Name, c.Instruction, got, c.Bits, c.Width)
			}
		}()
	}
	if len(seen) != 196 {
		t.Fatalf("covered %d opcodes, want 196", len(seen))
	}
}
func TestMutationWidthsAndMetadata(t *testing.T) {
	i := Instruction{Kind: Addi, Rd: A0, Rs1: Zero, Imm: 6}
	copyI := i
	i.Imm = 42
	if i.Encode().Bits != 0x02a00513 || copyI.Encode().Bits == i.Encode().Bits {
		t.Fatal("operand mutation or copying failed")
	}
	instructions := []Instruction{i, Compressed(1), Standard(0xdeadbeef)}
	var bytes []byte
	for _, inst := range instructions {
		bytes = inst.AppendBytes(bytes)
	}
	want := []byte{0x13, 5, 0xa0, 2, 1, 0, 0xef, 0xbe, 0xad, 0xde}
	if !reflect.DeepEqual(bytes, want) {
		t.Fatalf("mixed widths: %x", bytes)
	}
	if instructions[1].String() != ".2byte 0x0001" || i.String() != ".4byte 0x02a00513" {
		t.Fatal("assembly rendering")
	}
	d, ok := SyntacticDependencyFor("lw")
	if !ok || len(d.Sources) == 0 || d.Sources[0] != RegTypeAddress {
		t.Fatal("load dependency metadata")
	}
	if _, ok := Standard(0).SyntacticDependency(); ok {
		t.Fatal("raw instruction has metadata")
	}
	if IntReg(A0).ABIName() != "a0" || FloatReg(Ft11).ABIName() != "ft11" || CSR_MTVEC != 0x305 {
		t.Fatal("architectural metadata")
	}
	for _, kind := range AllKinds() {
		if kind.Mnemonic() == "" {
			t.Fatal("missing mnemonic")
		}
	}
}
func TestImmediateUtilities(t *testing.T) {
	for _, value := range []uint64{0, 0x7ff, 0x800, 0xfff, 0x12345678, 0x7fffffff, 0x80000000, 0xffffffff} {
		hi, lo := LiIntoReg(value, false)
		if uint32(hi<<12+uint32(lo)) != uint32(value) {
			t.Fatalf("cannot reconstruct %x", value)
		}
	}
	if TwosComplement(0xffffffff, false) != -1 || ToUnsigned(-1, false) != 0xffffffff || TwosComplement(^uint64(0), true) != -1 {
		t.Fatal("signed conversion")
	}
}
func TestAtomicOrdering(t *testing.T) {
	for aq := 0; aq < 2; aq++ {
		for rl := 0; rl < 2; rl++ {
			i := Instruction{Kind: LrW, Rd: T0, Rs1: T1, Aq: aq != 0, Rl: rl != 0}
			bits := i.Encode().Bits
			if bits>>26&1 != uint32(aq) || bits>>25&1 != uint32(rl) || bits>>20&31 != 0 {
				t.Fatal("atomic ordering or LR rs2")
			}
		}
	}
}

func TestInvalidRegistersAreRejected(t *testing.T) {
	for _, rd := range []int32{-1, 32} {
		func() {
			defer func() {
				if recover() == nil {
					t.Errorf("accepted register %d", rd)
				}
			}()
			Rv32i_add(rd, 0, 0)
		}()
	}
}
