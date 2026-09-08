// Package riscv provides RISC-V instruction encoding and architectural metadata.
package riscv

import (
	"encoding/binary"
	"fmt"
	"math/rand"
)

// Instruction stores editable operands. Rd/Rs1/Rs2/Rs3 are register numbers;
// the opcode determines whether each refers to an integer or floating register.
// Immediates follow the low-level encoders: branch offsets are bytes, and LUI
// takes the upper 20-bit field. Irrelevant fields are ignored.
type Instruction struct {
	Kind                      InstructionKind
	Rd, Rs1, Rs2, Rs3         int32
	Imm, Shamt, Rm, Csr, Uimm int32
	Aq, Rl                    bool
	Raw                       uint32
}

type EncodedInstruction struct {
	Bits  uint32
	Width int
}

func Standard(word uint32) Instruction       { return Instruction{Kind: Raw32, Raw: word} }
func Compressed(halfword uint16) Instruction { return Instruction{Kind: Raw16, Raw: uint32(halfword)} }
func (i Instruction) Encode() EncodedInstruction {
	info := instructionTable[i.Kind]
	word := info.encode(i)
	if info.width == 2 && word > 0xffff {
		panic("compressed encoding exceeds 16 bits")
	}
	return EncodedInstruction{word, info.width}
}
func (i Instruction) ByteLen() int            { return instructionTable[i.Kind].width }
func (i Instruction) Mnemonic() string        { return i.Kind.Mnemonic() }
func (i Instruction) Class() InstructionClass { return i.Kind.Class() }
func (i Instruction) SyntacticDependency() (SyntacticDependency, bool) {
	return SyntacticDependencyFor(i.Mnemonic())
}
func (i Instruction) AppendBytes(dst []byte) []byte { return i.Encode().AppendBytes(dst) }
func (e EncodedInstruction) AppendBytes(dst []byte) []byte {
	if e.Width == 2 {
		if e.Bits > 0xffff {
			panic("compressed encoding exceeds 16 bits")
		}
		return binary.LittleEndian.AppendUint16(dst, uint16(e.Bits))
	}
	if e.Width != 4 {
		panic("instruction width must be 2 or 4")
	}
	return binary.LittleEndian.AppendUint32(dst, e.Bits)
}
func (e EncodedInstruction) String() string {
	return fmt.Sprintf(".%dbyte 0x%0*x", e.Width, e.Width*2, e.Bits)
}
func (i Instruction) String() string { return i.Encode().String() }
func assert(ok bool) {
	if !ok {
		panic("invalid instruction operand")
	}
}
func boolInt(b bool) int32 {
	if b {
		return 1
	}
	return 0
}

var intRegisters = [...]int32{5, 6, 7, 28, 29, 30, 31, 10, 11, 12, 13, 14, 15, 16, 17}
var floatRegisters = [...]int32{0, 1, 2, 3, 4, 5, 6, 7, 28, 29, 30, 31, 10, 11, 12, 13, 14, 15, 16, 17}

func RandomIntRegister(rng *rand.Rand) int32 { return intRegisters[rng.Intn(len(intRegisters))] }

// RandomInstruction supplies encodable operands. It does not provide a memory
// map or machine state: loads, indirect jumps, and CSR accesses may trap.
func RandomInstruction(kind InstructionKind, rng *rand.Rand, is64 bool) Instruction {
	i := Instruction{Kind: kind}
	for _, op := range instructionTable[kind].operands {
		var value int32
		switch op.field {
		case fieldRd, fieldRs1, fieldRs2, fieldRs3:
			if op.floating {
				value = floatRegisters[rng.Intn(len(floatRegisters))]
			} else {
				value = RandomIntRegister(rng)
			}
		case fieldImm:
			switch kind {
			case Lui, Auipc:
				value = int32(rng.Intn(1 << 20))
			case Fence:
				value = int32(rng.Intn(256))
			case FenceI:
				value = 0
			case Jal:
				value = 4
			case Jalr:
				value = int32(rng.Intn(1024)-512) * 4
			default:
				switch kind.Class() {
				case ClassBranch:
					value = 4
				case ClassMemory, ClassMemory64, ClassFloatMemory, ClassDoubleMemory:
					value = int32(rng.Intn(512)-256) * 8
				default:
					value = int32(rng.Intn(4096) - 2048)
				}
			}
		case fieldShamt:
			width := 32
			if is64 && kind.Class() == ClassAlu {
				width = 64
			}
			value = int32(rng.Intn(width))
		case fieldRm:
			value = RoundingModes[rng.Intn(len(RoundingModes))]
		case fieldCsr:
			value = int32(rng.Intn(4096))
		case fieldUimm:
			value = int32(rng.Intn(32))
		case fieldAq, fieldRl:
			value = int32(rng.Intn(2))
		}
		switch op.field {
		case fieldRd:
			i.Rd = value
		case fieldRs1:
			i.Rs1 = value
		case fieldRs2:
			i.Rs2 = value
		case fieldRs3:
			i.Rs3 = value
		case fieldImm:
			i.Imm = value
		case fieldShamt:
			i.Shamt = value
		case fieldRm:
			i.Rm = value
		case fieldCsr:
			i.Csr = value
		case fieldUimm:
			i.Uimm = value
		case fieldAq:
			i.Aq = value != 0
		case fieldRl:
			i.Rl = value != 0
		}
	}
	return i
}
