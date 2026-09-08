package rvgen

import (
	"bytes"
	"context"
	"crypto/sha256"
	"debug/elf"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"rvgen/riscv"
	"testing"
	"time"
)

func readELF(t *testing.T, data []byte) *elf.File {
	t.Helper()
	f, err := elf.NewFile(bytes.NewReader(data))
	if err != nil {
		t.Fatal(err)
	}
	return f
}
func TestRuntimeRustParityAndELF(t *testing.T) {
	fixture, err := os.ReadFile("testdata/rust_runtime.json")
	if err != nil {
		t.Fatal(err)
	}
	var hashes map[string]string
	if err = json.Unmarshal(fixture, &hashes); err != nil {
		t.Fatal(err)
	}
	for _, is64 := range []bool{false, true} {
		for name, body := range map[string][]byte{"empty": {}, "compressed": {1, 0}, "illegal": {255, 255, 255, 255}, "long": bytes.Repeat([]byte{0x13, 0, 0, 0}, 2048)} {
			data, err := Executable(body, is64, 0x80000100)
			if err != nil {
				t.Fatal(err)
			}
			key := fmt.Sprintf("%t-%s", is64, name)
			if got := fmt.Sprintf("%x", sha256.Sum256(data)); got != hashes[key] {
				t.Errorf("%s differs from Rust ELF: %s", key, got)
			}
			f := readELF(t, data)
			if f.Entry != 0x80000100 || f.Machine != elf.EM_RISCV || f.Type != elf.ET_EXEC || (f.Class == elf.ELFCLASS64) != is64 {
				t.Fatal("ELF header")
			}
			if len(f.Progs) != 3 {
				t.Fatal("load segments")
			}
			symbols, err := f.Symbols()
			if err != nil {
				t.Fatal(err)
			}
			for _, name := range []string{"tohost", "fromhost"} {
				s := f.Section("." + name)
				if s == nil {
					t.Fatal("missing mailbox")
				}
				contents, err := s.Data()
				if err != nil {
					t.Fatal(err)
				}
				if s.Addr%64 != 0 || s.Flags != elf.SHF_ALLOC|elf.SHF_WRITE || !bytes.Equal(contents, make([]byte, 8)) {
					t.Fatal("mailbox layout")
				}
				found := false
				for _, sym := range symbols {
					if sym.Name == name {
						found = true
						if sym.Value != s.Addr || sym.Size != 8 || elf.ST_BIND(sym.Info) != elf.STB_GLOBAL {
							t.Fatal("mailbox symbol")
						}
					}
				}
				if !found {
					t.Fatal("missing symbol")
				}
			}
		}
	}
}
func TestELFSectionsAndValidation(t *testing.T) {
	code := NewELFSection(".text", []byte{0x13, 0, 0, 0})
	code.Addr = 0x80000000
	data := NewELFSection(".data", []byte{1, 2, 3})
	data.Addr = 0x80001000
	data.Align = 16
	data.Flags = SHFAlloc | SHFWrite
	comment := NewELFSection(".comment", []byte("fixture"))
	comment.Flags = 0
	comment.Align = 1
	sections := []ELFSection{code, data, comment}
	for _, is64 := range []bool{false, true} {
		b, err := BuildELF(sections, is64, code.Addr)
		if err != nil {
			t.Fatal(err)
		}
		f := readELF(t, b)
		if len(f.Progs) != 2 {
			t.Fatal("non-allocated section has segment")
		}
		for _, want := range sections {
			s := f.Section(want.Name)
			got, err := s.Data()
			if err != nil {
				t.Fatal(err)
			}
			if !bytes.Equal(got, want.Bytes) || s.Addr != want.Addr || s.Offset%want.Align != 0 {
				t.Fatal("section layout")
			}
		}
	}
	for _, mutate := range []func(*ELFSection){
		func(s *ELFSection) { s.Align = 3 }, func(s *ELFSection) { s.Addr = ^uint64(0) },
		func(s *ELFSection) { s.Name = "" }, func(s *ELFSection) { s.Name = ".shstrtab" },
		func(s *ELFSection) { s.Name = "bad\x00name" }, func(s *ELFSection) { s.Name = "é" }, func(s *ELFSection) { s.Align = 1 << 62 },
	} {
		s := code
		mutate(&s)
		if _, err := BuildELF([]ELFSection{s}, true, 0); err == nil {
			t.Fatalf("accepted invalid section %+v", s)
		}
	}
	if _, err := BuildELF([]ELFSection{code, code}, true, 0); err == nil {
		t.Fatal("duplicate section")
	}
	if _, err := BuildELF(nil, false, 1<<32); err == nil {
		t.Fatal("ELF32 entry overflow")
	}
	b, err := BuildELF(nil, true, 0)
	if err != nil {
		t.Fatal(err)
	}
	if len(readELF(t, b).Progs) != 0 {
		t.Fatal("empty ELF")
	}
	code.Addr = 0xffffffff80000000
	b, err = BuildELF([]ELFSection{code}, true, code.Addr)
	if err != nil {
		t.Fatal(err)
	}
	if readELF(t, b).Entry != code.Addr {
		t.Fatal("64-bit address")
	}
	for _, s := range []ELFSymbol{{"", 0, 0, 1, 1}, {"x", 1, 0, 1, 1}, {"x", 0, 4, 1, 1}, {"x", 0, 0, 1, 16}, {"x", -1, 0, 1, 1}} {
		if _, err := BuildELFWithSymbols([]ELFSection{code}, []ELFSymbol{s}, true, 0); err == nil {
			t.Fatal("invalid symbol")
		}
	}
	for _, c := range []struct {
		body  []byte
		is64  bool
		start uint64
	}{{nil, true, 0x80000002}, {[]byte{0}, true, 0x80000000}, {nil, false, 0xfffffff0}, {nil, true, ^uint64(0) - 3}} {
		if _, err := Executable(c.body, c.is64, c.start); err == nil {
			t.Fatal("invalid runtime layout")
		}
	}
}
func TestSpikeRuntime(t *testing.T) {
	spike := os.Getenv("SPIKE")
	if spike == "" {
		spike = "spike"
	}
	if _, err := exec.LookPath(spike); err != nil {
		t.Skip("Spike unavailable")
	}
	for _, is64 := range []bool{false, true} {
		p := DefaultParams()
		p.Is64Bit = is64
		p.Size = 511
		p.NumBBs = 7
		p.Seed = 1729
		g, err := NewGenerator(p)
		if err != nil {
			t.Fatal(err)
		}
		g.ChoiceWeights = nil
		for _, c := range []riscv.InstructionClass{riscv.ClassAlu, riscv.ClassMulDiv, riscv.ClassBranch, riscv.ClassJal, riscv.ClassFence} {
			g.ChoiceWeights = append(g.ChoiceWeights, ChoiceWeight{InstructionChoice(c), 1})
		}
		g.ChoiceWeights = append(g.ChoiceWeights, ChoiceWeight{ActionChoice(RegFsm), 1})
		if err = g.Generate(); err != nil {
			t.Fatal(err)
		}
		for name, body := range map[string][]byte{"empty": {}, "compressed": {1, 0}, "illegal": {255, 255, 255, 255}, "long": bytes.Repeat([]byte{0x13, 0, 0, 0}, 2048), "generated": g.Cores[0].Bytecode()} {
			data, err := Executable(body, is64, 0x80000100)
			if err != nil {
				t.Fatal(err)
			}
			file := filepath.Join(t.TempDir(), "case.elf")
			if err = os.WriteFile(file, data, 0644); err != nil {
				t.Fatal(err)
			}
			isa := "--isa=rv32gc"
			if is64 {
				isa = "--isa=rv64gc"
			}
			ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
			cmd := exec.CommandContext(ctx, spike, "-m64", isa, file)
			output, err := cmd.CombinedOutput()
			cancel()
			want := 0
			if name == "illegal" {
				want = 1
			}
			if cmd.ProcessState == nil || cmd.ProcessState.ExitCode() != want {
				t.Fatalf("%t %s: %v %s", is64, name, err, output)
			}
		}
	}
}
