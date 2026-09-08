package rvgen

import (
	"fmt"
	"math"
	"math/rand"
	"rvgen/riscv"
)

type GenerationAction uint8

const (
	RegFsm GenerationAction = iota
	FpuFsm
	Exception
	DescendPrivilege
	WaitForInterruptTrap
	WaitForInterruptNoTrap
	SendIpi
	SendLocalInterrupt
	ClearInterrupt
	EpcFsm
	TvecFsm
	MachineCsr
	FreePolluted
	CreateAddressDependency
	ClearMdt
	ResetMcm
	WriteInstruction
	WaitForInstruction
)

// A choice selects either a class or an action. Use the two constructors below.
type GenerationChoice struct {
	Class  riscv.InstructionClass
	Action GenerationAction
}

func InstructionChoice(class riscv.InstructionClass) GenerationChoice {
	return GenerationChoice{Class: class}
}
func ActionChoice(action GenerationAction) GenerationChoice { return GenerationChoice{Action: action} }

type ChoiceWeight struct {
	Choice GenerationChoice
	Weight float64
}

func DefaultChoiceWeights(is64, privileged bool) []ChoiceWeight {
	classes := []struct {
		class  riscv.InstructionClass
		weight float64
	}{
		{riscv.ClassAlu, .1}, {riscv.ClassAlu64, .1}, {riscv.ClassMulDiv, .1}, {riscv.ClassMulDiv64, .1},
		{riscv.ClassJal, .01}, {riscv.ClassJalr, .01}, {riscv.ClassBranch, .4}, {riscv.ClassMemory, .5}, {riscv.ClassMemory64, .5},
		{riscv.ClassAmo, 0}, {riscv.ClassAmo64, 0}, {riscv.ClassFence, 1e-6},
		{riscv.ClassFloatMemory, 0}, {riscv.ClassDoubleMemory, 0}, {riscv.ClassFloat, 0}, {riscv.ClassFloat64, 0},
		{riscv.ClassDouble, 0}, {riscv.ClassDouble64, 0}, {riscv.ClassCsr, 0},
	}
	weights := make([]ChoiceWeight, 0, 37)
	for _, c := range classes {
		weight := c.weight
		if (!is64 && c.class.RequiresRV64()) || (!privileged && c.class == riscv.ClassCsr) {
			weight = 0
		}
		weights = append(weights, ChoiceWeight{InstructionChoice(c.class), weight})
	}
	for _, a := range []struct {
		action GenerationAction
		weight float64
	}{
		{RegFsm, .1}, {CreateAddressDependency, 2}, {Exception, .1}, {FreePolluted, 0}, {FpuFsm, 0}, {TvecFsm, 0},
		{EpcFsm, 0}, {MachineCsr, 0}, {DescendPrivilege, 0}, {WaitForInterruptTrap, 0}, {WaitForInterruptNoTrap, 0},
		{SendIpi, 0}, {SendLocalInterrupt, 0}, {ClearInterrupt, 0}, {WriteInstruction, 0}, {WaitForInstruction, 0}, {ClearMdt, 0}, {ResetMcm, 0},
	} {
		weights = append(weights, ChoiceWeight{ActionChoice(a.action), a.weight})
	}
	return weights
}
func weightTotal(weights []ChoiceWeight) (float64, error) {
	total := 0.0
	for _, w := range weights {
		if math.IsNaN(w.Weight) || math.IsInf(w.Weight, 0) || w.Weight < 0 {
			return 0, fmt.Errorf("choice weights must be finite and nonnegative")
		}
		total += w.Weight
	}
	if math.IsInf(total, 0) || total <= 0 {
		return 0, fmt.Errorf("choice weights must have a finite positive total")
	}
	return total, nil
}
func (c GenerationChoice) Validate(p GeneratorParams) error {
	if c.Class != riscv.NoClass {
		if c.Class > riscv.ClassCsr {
			return fmt.Errorf("unknown instruction class %d", c.Class)
		}
		if c.Class.RequiresRV64() && !p.Is64Bit {
			return fmt.Errorf("instruction class %d requires RV64", c.Class)
		}
		if c.Class == riscv.ClassCsr && !p.AuthorizePrivileges {
			return fmt.Errorf("CSR generation requires authorize_privileges")
		}
		return nil
	}
	switch c.Action {
	case RegFsm, Exception, FreePolluted, CreateAddressDependency:
		return nil
	}
	return fmt.Errorf("action %d requires platform/state support; lowering is not implemented", c.Action)
}

var classKinds = func() map[riscv.InstructionClass][]riscv.InstructionKind {
	result := make(map[riscv.InstructionClass][]riscv.InstructionKind)
	for _, kind := range riscv.AllKinds() {
		result[kind.Class()] = append(result[kind.Class()], kind)
	}
	return result
}()

// lower writes at most two instructions to caller-owned storage to avoid a
// separate heap allocation for every sampled instruction or action.
func (c GenerationChoice) lower(rng *rand.Rand, is64 bool, dst *[2]riscv.Instruction) int {
	if c.Class != riscv.NoClass {
		kinds := classKinds[c.Class]
		dst[0] = riscv.RandomInstruction(kinds[rng.Intn(len(kinds))], rng, is64)
		return 1
	}
	switch c.Action {
	case FreePolluted:
		return 0
	case Exception:
		kind := riscv.Ebreak
		if rng.Intn(2) != 0 {
			kind = riscv.Ecall
		}
		dst[0] = riscv.Instruction{Kind: kind}
		return 1
	case RegFsm:
		rd := riscv.RandomIntRegister(rng)
		hi, lo := riscv.LiIntoReg(uint64(rng.Uint32()), false)
		dst[0] = riscv.Instruction{Kind: riscv.Lui, Rd: rd, Imm: int32(hi)}
		kind := riscv.Addi
		if is64 {
			kind = riscv.Addiw
		}
		dst[1] = riscv.Instruction{Kind: kind, Rd: rd, Rs1: rd, Imm: lo}
		return 2
	case CreateAddressDependency:
		source := riscv.RandomIntRegister(rng)
		address := riscv.RandomIntRegister(rng)
		for address == source {
			address = riscv.RandomIntRegister(rng)
		}
		dst[0] = riscv.Instruction{Kind: riscv.Addi, Rd: address, Rs1: source}
		dst[1] = riscv.Instruction{Kind: riscv.Lw, Rd: riscv.RandomIntRegister(rng), Rs1: address}
		return 2
	}
	panic("unvalidated generation choice")
}
func (c GenerationChoice) Lower(rng *rand.Rand, p GeneratorParams) ([]riscv.Instruction, error) {
	if err := c.Validate(p); err != nil {
		return nil, err
	}
	var dst [2]riscv.Instruction
	n := c.lower(rng, p.Is64Bit, &dst)
	return dst[:n], nil
}
