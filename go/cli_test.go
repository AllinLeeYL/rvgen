package rvgen

import (
	"bytes"
	"errors"
	"flag"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"runtime"
	"strings"
	"testing"
)

var testBinary string

func TestMain(m *testing.M) {
	dir, err := os.MkdirTemp("", "rvgen-go-test-")
	if err != nil {
		panic(err)
	}
	testBinary = filepath.Join(dir, "rvgen")
	cmd := exec.Command(filepath.Join(runtime.GOROOT(), "bin", "go"), "build", "-o", testBinary, "./cmd/rvgen")
	if output, err := cmd.CombinedOutput(); err != nil {
		os.RemoveAll(dir)
		panic(string(output) + err.Error())
	}
	code := m.Run()
	os.RemoveAll(dir)
	os.Exit(code)
}
func TestCLIConfigPrecedence(t *testing.T) {
	c, err := ParseCLI(nil, io.Discard)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(c.Params(), DefaultParams()) || c.Out != "output.elf" || c.OutDir != "elfs" {
		t.Fatal("defaults")
	}
	dir := t.TempDir()
	config := filepath.Join(dir, "config.toml")
	if err = os.WriteFile(config, []byte("size=10\nmemsize=8192\nnum_cores=2\nnum_bbs=3\nseed=7\nauthorize_privileges=true\nout='configured.elf'\nnum_elfs=2\nout_dir='batch'"), 0644); err != nil {
		t.Fatal(err)
	}
	c, err = ParseCLI([]string{"--config", config, "--size", "0", "--num-cores=1", "--seed", "-42", "--authorize-privileges", "false", "-o", "explicit.elf", "--num-elfs=3", "--out-dir=chosen"}, io.Discard)
	if err != nil {
		t.Fatal(err)
	}
	if c.Size != 0 || c.NumCores != 1 || c.NumBBs != 3 || c.Memsize != 8192 || c.Seed != -42 || c.AuthorizePrivileges || c.Out != "explicit.elf" || c.NumELFs != 3 || c.OutDir != "chosen" {
		t.Fatalf("overrides %+v", c)
	}
	if c.OutputPath(2) != filepath.Join("chosen", "explicit_000002.elf") {
		t.Fatal("batch path")
	}
	c.Out = "prefix/case"
	c.OutDir = "prefix"
	if c.OutputPath(0) != filepath.Join("prefix", "case_000000.elf") {
		t.Fatal("extension default")
	}
	for _, value := range []string{"1", "true", "YES", "TrUe"} {
		if !ParseBool(value) {
			t.Fatal(value)
		}
	}
	for _, value := range []string{"0", "false", "anything", ""} {
		if ParseBool(value) {
			t.Fatal(value)
		}
	}
}
func TestCLIInvalidInputsAndHelp(t *testing.T) {
	for _, source := range []string{"unknown=1", "config='nested.toml'", "size='12'", "size=1.0", "num_cores=-1", "seed=true", "authorize_privileges='true'", "[instruction_weights]\nALU=0.1", "size=1\nsize=2"} {
		if _, err := DecodeConfig(source); err == nil {
			t.Fatalf("accepted %s", source)
		}
	}
	for _, args := range [][]string{{"--size", "-1"}, {"--size", "abc"}, {"--size", "0xff"}, {"--seed", "0xff"}, {"--unknown", "1"}, {"--num-cores", "0"}, {"--num-bbs", "0"}, {"--num-elfs", "0"}, {"--num-elfs", "2", "--seed", "9223372036854775807"}, {"unexpected"}, {"--config", "missing.toml"}} {
		if _, err := ParseCLI(args, io.Discard); err == nil {
			t.Fatalf("accepted %v", args)
		}
	}
	var output bytes.Buffer
	if _, err := ParseCLI([]string{"--config", "missing.toml", "--help"}, &output); !errors.Is(err, flag.ErrHelp) {
		t.Fatal(err)
	}
	if !strings.Contains(output.String(), "num-bbs") {
		t.Fatal("help")
	}
	output.Reset()
	if _, err := ParseCLI([]string{"--version"}, &output); !errors.Is(err, flag.ErrHelp) || !strings.Contains(output.String(), Version) {
		t.Fatal("version")
	}
}
func TestBinaryConfigBatchAndErrors(t *testing.T) {
	dir := t.TempDir()
	config := []byte("size=1\nnum_bbs=2\nout='from config.elf'\nnum_elfs=2\nout_dir='wrong'")
	if err := os.WriteFile(filepath.Join(dir, "config.toml"), config, 0644); err != nil {
		t.Fatal(err)
	}
	cmd := exec.Command(testBinary, "--config=config.toml", "--size=0", "--num-elfs=3", "--out-dir=nested/output")
	cmd.Dir = dir
	output, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("%v: %s", err, output)
	}
	if !strings.Contains(string(output), "Generated 3 ELF(s)") {
		t.Fatal("throughput output")
	}
	if _, err = os.Stat(filepath.Join(dir, "wrong")); !os.IsNotExist(err) {
		t.Fatal("config overrode explicit directory")
	}
	for i := 0; i < 3; i++ {
		c := DefaultCLI()
		c.Out = "from config.elf"
		c.OutDir = filepath.Join(dir, "nested/output")
		c.NumELFs = 3
		data, err := os.ReadFile(c.OutputPath(i))
		if err != nil {
			t.Fatal(err)
		}
		if readELF(t, data).Section(".tohost") == nil {
			t.Fatal("not executable")
		}
	}
	sentinel := filepath.Join(dir, "output.elf")
	if err = os.WriteFile(sentinel, []byte("keep existing file"), 0644); err != nil {
		t.Fatal(err)
	}
	for _, args := range [][]string{{"--num-cores", "2"}, {"--config", "missing.toml"}, {"--out-dir", "output.elf"}} {
		cmd = exec.Command(testBinary, args...)
		cmd.Dir = dir
		if out, err := cmd.CombinedOutput(); err == nil || len(out) == 0 {
			t.Fatalf("expected error: %v %s", args, out)
		}
		data, err := os.ReadFile(sentinel)
		if err != nil {
			t.Fatal(err)
		}
		if string(data) != "keep existing file" {
			t.Fatal("error overwrote output")
		}
	}
	cmd = exec.Command(testBinary, "--config", "missing.toml", "--help")
	cmd.Dir = dir
	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("help: %v %s", err, out)
	}
}
