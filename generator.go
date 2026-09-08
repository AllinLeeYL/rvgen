package rvgen

import (
	_ "embed"
	"fmt"
	"math/rand"
	"os"
	"rvgen/riscv"
	"strings"
)

// SelectedChoice records the half-open instruction range emitted by a choice.
// Callers editing Instructions must update or discard these generation records.
type SelectedChoice struct {
	Choice     GenerationChoice
	Start, End int
}
type BasicBlock struct {
	Label           string
	Instructions    []riscv.Instruction
	SelectedChoices []SelectedChoice
}

// generateBlock receives a validated policy and a derived instruction budget.
// The block belongs to the pending workload until Generate succeeds.
func generateBlock(b *BasicBlock, budget int, rng *rand.Rand, weights []ChoiceWeight, is64 bool) error {
	insts := make([]riscv.Instruction, 0, budget)
	selected := make([]SelectedChoice, 0, budget)
	eligible := make([]ChoiceWeight, len(weights))
	for len(insts) < budget {
		copy(eligible, weights)
		for {
			total, err := weightTotal(eligible)
			if err != nil {
				return fmt.Errorf("cannot fill block %s: no choice fits remaining budget of %d", b.Label, budget-len(insts))
			}
			target := rng.Float64() * total
			index := -1
			for j, w := range eligible {
				if w.Weight == 0 {
					continue
				}
				index = j
				if target < w.Weight {
					break
				}
				target -= w.Weight
			}
			choice := eligible[index].Choice
			var lowered [2]riscv.Instruction
			n := choice.lower(rng, is64, &lowered)
			if n > budget-len(insts) {
				eligible[index].Weight = 0
				continue
			}
			start := len(insts)
			insts = append(insts, lowered[:n]...)
			selected = append(selected, SelectedChoice{choice, start, len(insts)})
			if n != 0 {
				break
			}
			for j, w := range eligible {
				if w.Choice == choice {
					eligible[j].Weight = 0
				}
			}
		}
	}
	b.Instructions = insts
	b.SelectedChoices = selected
	return nil
}

type Core struct {
	Blocks []BasicBlock
}

func (c Core) Bytecode() []byte {
	count := 0
	for _, b := range c.Blocks {
		count += len(b.Instructions)
	}
	bytes := make([]byte, 0, count*4)
	for _, b := range c.Blocks {
		for _, inst := range b.Instructions {
			bytes = inst.AppendBytes(bytes)
		}
	}
	return bytes
}

type Generator struct {
	Params        GeneratorParams
	Cores         []Core
	ChoiceWeights []ChoiceWeight
}

func NewGenerator(p GeneratorParams) (*Generator, error) {
	if err := p.Validate(); err != nil {
		return nil, err
	}
	return &Generator{p, createCores(p), DefaultChoiceWeights(p.Is64Bit, p.AuthorizePrivileges)}, nil
}
func createCores(p GeneratorParams) []Core {
	cores := make([]Core, p.NumCores)
	for i := range cores {
		cores[i].Blocks = make([]BasicBlock, p.NumBBs)
		for j := range cores[i].Blocks {
			cores[i].Blocks[j].Label = fmt.Sprintf("bb_%d", j)
		}
	}
	return cores
}

// Generate replaces the entire workload only after successful generation.
func (g *Generator) Generate() error {
	if err := g.Params.Validate(); err != nil {
		return err
	}
	if _, err := weightTotal(g.ChoiceWeights); err != nil {
		return err
	}
	for _, w := range g.ChoiceWeights {
		if w.Weight > 0 {
			if err := w.Choice.Validate(g.Params); err != nil {
				return err
			}
		}
	}
	seed := g.Params.Seed
	if seed < 0 {
		seed = -seed
	}
	rng := rand.New(rand.NewSource(seed))
	cores := createCores(g.Params)
	for i := range cores {
		coreBudget := partition(g.Params.Size, len(cores), i)
		for j := range cores[i].Blocks {
			budget := partition(coreBudget, len(cores[i].Blocks), j)
			if err := generateBlock(&cores[i].Blocks[j], budget, rng, g.ChoiceWeights, g.Params.Is64Bit); err != nil {
				return err
			}
		}
	}
	g.Cores = cores
	return nil
}

//go:embed templates/riscv64.S
var assemblyTemplate string

func (g *Generator) Assembly() (string, error) {
	if len(g.Cores) == 0 {
		return "", fmt.Errorf("no cores to render")
	}
	var blocks strings.Builder
	for _, b := range g.Cores[0].Blocks {
		fmt.Fprintf(&blocks, "\n%s:\n    ", b.Label)
		for _, i := range b.Instructions {
			fmt.Fprintf(&blocks, "\n    %s\n    ", i)
		}
		blocks.WriteByte('\n')
	}
	return strings.ReplaceAll(assemblyTemplate, "{{ basic_blocks }}", blocks.String()), nil
}
func (g *Generator) GenAssembly(path string) error {
	s, err := g.Assembly()
	if err != nil {
		return err
	}
	return os.WriteFile(path, []byte(s), 0644)
}
func (g *Generator) ELF() ([]byte, error) {
	if len(g.Cores) != 1 {
		return nil, fmt.Errorf("only one core is supported for ELF output")
	}
	return Executable(g.Cores[0].Bytecode(), g.Params.Is64Bit, g.Params.StartAddr)
}
func (g *Generator) GenELF(path string) error {
	data, err := g.ELF()
	if err != nil {
		return err
	}
	return os.WriteFile(path, data, 0644)
}
