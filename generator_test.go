package rvgen

import (
	"bytes"
	"math"
	"math/rand"
	"reflect"
	"rvgen/riscv"
	"strings"
	"testing"
)

func mustGenerator(t *testing.T, p GeneratorParams) *Generator {
	t.Helper()
	g, err := NewGenerator(p)
	if err != nil {
		t.Fatal(err)
	}
	return g
}
func TestBudgetsSeedsAndRegeneration(t *testing.T) {
	p := DefaultParams()
	p.Size = 21
	p.NumCores = 2
	p.NumBBs = 3
	g := mustGenerator(t, p)
	if err := g.Generate(); err != nil {
		t.Fatal(err)
	}
	for c, counts := range [][]int{{4, 4, 3}, {4, 3, 3}} {
		for b, count := range counts {
			if len(g.Cores[c].Blocks[b].Instructions) != count {
				t.Fatal("incorrect budget partition")
			}
		}
	}
	if _, err := g.ELF(); err == nil {
		t.Fatal("multicore ELF accepted")
	}
	p.NumCores = 1
	p.Size = 257
	p.NumBBs = 17
	g = mustGenerator(t, p)
	if err := g.Generate(); err != nil {
		t.Fatal(err)
	}
	first := g.Cores[0].Bytecode()
	if err := g.Generate(); err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(first, g.Cores[0].Bytecode()) {
		t.Fatal("same seed differs")
	}
	g.Params.Seed = 42
	if err := g.Generate(); err != nil {
		t.Fatal(err)
	}
	positive := g.Cores[0].Bytecode()
	if bytes.Equal(first, positive) {
		t.Fatal("different seeds match")
	}
	g.Params.Seed = -42
	if err := g.Generate(); err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(positive, g.Cores[0].Bytecode()) {
		t.Fatal("absolute seed convention")
	}
	g.Params.Size = 3
	g.Params.NumBBs = 12
	if err := g.Generate(); err != nil {
		t.Fatal(err)
	}
	if len(g.Cores[0].Blocks) != 12 || len(g.Cores[0].Bytecode()) != 12 {
		t.Fatal("layout was not regenerated")
	}
	g.Params.Size = 0
	if err := g.Generate(); err != nil {
		t.Fatal(err)
	}
	if len(g.Cores[0].Bytecode()) != 0 {
		t.Fatal("zero budget")
	}
}
func TestActionsStayWholeAndFailuresRetainWorkload(t *testing.T) {
	p := DefaultParams()
	p.Size = 8
	p.NumBBs = 2
	g := mustGenerator(t, p)
	g.ChoiceWeights = []ChoiceWeight{{ActionChoice(RegFsm), 1}}
	if err := g.Generate(); err != nil {
		t.Fatal(err)
	}
	for _, b := range g.Cores[0].Blocks {
		for _, s := range b.SelectedChoices {
			if s.End-s.Start != 2 {
				t.Fatal("truncated sequence")
			}
		}
	}
	before := g.Cores[0].Bytecode()
	g.Params.Size = 7
	if err := g.Generate(); err == nil {
		t.Fatal("unfillable budget accepted")
	}
	if !bytes.Equal(before, g.Cores[0].Bytecode()) {
		t.Fatal("failed generation replaced instructions")
	}
	for _, weights := range [][]ChoiceWeight{
		{{ActionChoice(FreePolluted), 1}, {ActionChoice(FreePolluted), 1}},
		{{ActionChoice(SendIpi), 1}}, {{InstructionChoice(riscv.ClassAlu), math.NaN()}},
		{{InstructionChoice(riscv.ClassAlu), -1}}, {{InstructionChoice(riscv.ClassAlu), math.Inf(1)}},
		{{InstructionChoice(riscv.ClassAlu), 0}}, nil,
	} {
		g.ChoiceWeights = weights
		if err := g.Generate(); err == nil {
			t.Fatal("invalid policy accepted")
		}
		if !bytes.Equal(before, g.Cores[0].Bytecode()) {
			t.Fatal("failed policy replaced workload")
		}
	}
	g.ChoiceWeights = []ChoiceWeight{{ActionChoice(FreePolluted), 10}, {InstructionChoice(riscv.ClassAlu), 1}}
	if err := g.Generate(); err != nil {
		t.Fatal(err)
	}
	for _, b := range g.Cores[0].Blocks {
		lastEmpty := false
		for _, s := range b.SelectedChoices {
			empty := s.Start == s.End
			if lastEmpty && empty {
				t.Fatal("repeated no-op without progress")
			}
			lastEmpty = empty
		}
	}
	rng := rand.New(rand.NewSource(1))
	params := DefaultParams()
	insts, err := ActionChoice(CreateAddressDependency).Lower(rng, params)
	if err != nil {
		t.Fatal(err)
	}
	if len(insts) != 2 || insts[0].Kind != riscv.Addi || insts[1].Kind != riscv.Lw || insts[0].Rd != insts[1].Rs1 || insts[0].Rd == insts[0].Rs1 {
		t.Fatal("address dependency sequence")
	}
}
func TestEverySampledClassAndConstraints(t *testing.T) {
	for _, is64 := range []bool{false, true} {
		p := DefaultParams()
		p.Is64Bit = is64
		p.Size = 200
		p.NumBBs = 1
		for class := riscv.ClassAlu; class <= riscv.ClassCsr; class++ {
			g := mustGenerator(t, p)
			g.ChoiceWeights = []ChoiceWeight{{InstructionChoice(class), 1}}
			err := g.Generate()
			if !is64 && class.RequiresRV64() {
				if err == nil {
					t.Fatal("RV64 family on RV32")
				}
				continue
			}
			if err != nil {
				t.Fatal(err)
			}
			for _, i := range g.Cores[0].Blocks[0].Instructions {
				if i.Class() != class {
					t.Fatal("wrong class")
				}
				if i.Encode().Width != 4 {
					t.Fatal("compressed sampling")
				}
			}
		}
	}
	p := DefaultParams()
	p.AuthorizePrivileges = false
	g := mustGenerator(t, p)
	g.ChoiceWeights = []ChoiceWeight{{InstructionChoice(riscv.ClassCsr), 1}}
	if err := g.Generate(); err == nil {
		t.Fatal("unauthorized CSR")
	}
	g.ChoiceWeights = []ChoiceWeight{{ActionChoice(Exception), 1}}
	if err := g.Generate(); err != nil {
		t.Fatal(err)
	}
	other := mustGenerator(t, p)
	if reflect.DeepEqual(g.ChoiceWeights, other.ChoiceWeights) {
		t.Fatal("shared weights")
	}
	for _, mutate := range []func(*GeneratorParams){func(p *GeneratorParams) { p.NumCores = 0 }, func(p *GeneratorParams) { p.NumBBs = 0 }, func(p *GeneratorParams) { p.Size = -1 }, func(p *GeneratorParams) { p.Is64Bit = false; p.StartAddr = 1 << 32 }} {
		p := DefaultParams()
		mutate(&p)
		if _, err := NewGenerator(p); err == nil {
			t.Fatal("invalid parameters")
		}
	}
}

func TestGenerationFailurePreservesAllCores(t *testing.T) {
	p := DefaultParams()
	p.Size, p.NumCores, p.NumBBs = 8, 2, 1
	g := mustGenerator(t, p)
	g.ChoiceWeights = []ChoiceWeight{{ActionChoice(RegFsm), 1}}
	if err := g.Generate(); err != nil {
		t.Fatal(err)
	}
	// The first core can fill four instructions, but the second cannot fill three.
	// Neither the completed core nor any partial blocks should become visible.
	previous := mustGenerator(t, p)
	previous.ChoiceWeights = g.ChoiceWeights
	if err := previous.Generate(); err != nil {
		t.Fatal(err)
	}
	g.Params.Size = 7
	g.Params.Seed++
	if err := g.Generate(); err == nil {
		t.Fatal("unfillable second core accepted")
	}
	if !reflect.DeepEqual(g.Cores, previous.Cores) {
		t.Fatal("failed generation replaced the previous workload")
	}
	// Policies must still be checked when there are no instructions to sample.
	g.Params.Size = 0
	g.ChoiceWeights = []ChoiceWeight{{ActionChoice(SendIpi), 1}}
	if err := g.Generate(); err == nil {
		t.Fatal("empty workload bypassed policy validation")
	}
	if !reflect.DeepEqual(g.Cores, previous.Cores) {
		t.Fatal("invalid policy replaced the previous workload")
	}
}

func TestExplicitInstructionsAndAssembly(t *testing.T) {
	p := DefaultParams()
	p.NumBBs = 2
	g := mustGenerator(t, p)
	g.Cores[0].Blocks[0].Instructions = []riscv.Instruction{riscv.Compressed(1)}
	g.Cores[0].Blocks[1].Instructions = []riscv.Instruction{{Kind: riscv.Addi, Rd: riscv.A0, Imm: 42}}
	if got := g.Cores[0].Bytecode(); !bytes.Equal(got, []byte{1, 0, 0x13, 5, 0xa0, 2}) {
		t.Fatalf("block order %x", got)
	}
	asm, err := g.Assembly()
	if err != nil {
		t.Fatal(err)
	}
	for _, s := range []string{"bb_0:", "bb_1:", ".2byte 0x0001", ".4byte 0x02a00513"} {
		if !strings.Contains(asm, s) {
			t.Fatalf("missing %s", s)
		}
	}
	if strings.Contains(asm, "{{ basic_blocks }}") {
		t.Fatal("template not expanded")
	}
	if _, err = g.ELF(); err != nil {
		t.Fatal(err)
	}
}
func BenchmarkGenerate256(b *testing.B) {
	g, _ := NewGenerator(DefaultParams())
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		g.Params.Seed = int64(i)
		if err := g.Generate(); err != nil {
			b.Fatal(err)
		}
	}
}
func BenchmarkGenerateELF256(b *testing.B) {
	g, _ := NewGenerator(DefaultParams())
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		g.Params.Seed = int64(i)
		if err := g.Generate(); err != nil {
			b.Fatal(err)
		}
		if _, err := g.ELF(); err != nil {
			b.Fatal(err)
		}
	}
}
