package rvgen

import (
	"flag"
	"fmt"
	"io"
	"math"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"

	"github.com/BurntSushi/toml"
)

const Version = "0.1.0"

// CLI is shared by command-line parsing and TOML configuration. Only explicit
// flags override file settings; omitted values retain their defaults.
type CLI struct {
	Config              string `toml:"-"`
	Size                int    `toml:"size"`
	Memsize             int    `toml:"memsize"`
	NumCores            int    `toml:"num_cores"`
	NumBBs              int    `toml:"num_bbs"`
	Seed                int64  `toml:"seed"`
	AuthorizePrivileges bool   `toml:"authorize_privileges"`
	Out                 string `toml:"out"`
	NumELFs             int    `toml:"num_elfs"`
	OutDir              string `toml:"out_dir"`
}

func DefaultCLI() CLI {
	p := DefaultParams()
	return CLI{Size: p.Size, Memsize: p.Memsize, NumCores: p.NumCores, NumBBs: p.NumBBs, Seed: p.Seed, AuthorizePrivileges: p.AuthorizePrivileges, Out: "output.elf", NumELFs: 1, OutDir: "elfs"}
}
func ParseBool(value string) bool {
	switch strings.ToLower(value) {
	case "1", "true", "yes":
		return true
	}
	return false
}
func parseFlags(args []string, c CLI, output io.Writer) (CLI, error) {
	flags := flag.NewFlagSet("rvgen", flag.ContinueOnError)
	flags.SetOutput(output)
	flags.Usage = func() {
		fmt.Fprintln(output, "RISC-V instruction generator\n\nUsage: rvgen [OPTIONS]")
		flags.PrintDefaults()
	}
	count := func(name string, destination *int, usage string) {
		flags.Func(name, usage, func(value string) error {
			n, err := strconv.ParseUint(value, 10, strconv.IntSize-1)
			if err == nil {
				*destination = int(n)
			}
			return err
		})
		flags.Lookup(name).DefValue = strconv.Itoa(*destination)
	}
	flags.StringVar(&c.Config, "config", c.Config, "Read defaults from a TOML file")
	count("size", &c.Size, "Total workload instructions across all cores and blocks")
	count("memsize", &c.Memsize, "Reserved memory-size parameter")
	count("num-cores", &c.NumCores, "Number of cores; ELF output requires one")
	count("num-bbs", &c.NumBBs, "Number of basic blocks per core")
	flags.Func("seed", "Random seed", func(value string) error {
		seed, err := strconv.ParseInt(value, 10, 64)
		if err == nil {
			c.Seed = seed
		}
		return err
	})
	flags.Lookup("seed").DefValue = strconv.FormatInt(c.Seed, 10)
	flags.Func("authorize-privileges", "Allow privileged generation: 1, true, or yes (default true)", func(s string) error { c.AuthorizePrivileges = ParseBool(s); return nil })
	flags.StringVar(&c.Out, "out", c.Out, "Output ELF path")
	flags.StringVar(&c.Out, "o", c.Out, "Output ELF path (short form)")
	count("num-elfs", &c.NumELFs, "Number of ELFs to generate")
	flags.StringVar(&c.OutDir, "out-dir", c.OutDir, "Output directory, created if needed")
	var version bool
	flags.BoolVar(&version, "version", false, "Print version")
	flags.BoolVar(&version, "V", false, "Print version")
	if err := flags.Parse(args); err != nil {
		return CLI{}, err
	}
	if flags.NArg() != 0 {
		return CLI{}, fmt.Errorf("unexpected argument: %s", flags.Arg(0))
	}
	if version {
		fmt.Fprintf(output, "rvgen %s\n", Version)
		return CLI{}, flag.ErrHelp
	}
	return c, nil
}
func ParseCLI(args []string, output io.Writer) (CLI, error) {
	explicit, err := parseFlags(args, DefaultCLI(), output)
	if err != nil {
		return CLI{}, err
	}
	c := DefaultCLI()
	if explicit.Config != "" {
		source, err := os.ReadFile(explicit.Config)
		if err != nil {
			return CLI{}, fmt.Errorf("cannot read config %s: %w", explicit.Config, err)
		}
		c, err = DecodeConfig(string(source))
		if err != nil {
			return CLI{}, fmt.Errorf("invalid config %s: %w", explicit.Config, err)
		}
	}
	c, err = parseFlags(args, c, io.Discard)
	if err != nil {
		return CLI{}, err
	}
	if err = c.Validate(); err != nil {
		return CLI{}, err
	}
	return c, nil
}

// DecodeConfig applies TOML values to defaults and rejects unknown keys and
// incorrect types. Validation happens after CLI overrides have been applied.
func DecodeConfig(source string) (CLI, error) {
	c := DefaultCLI()
	metadata, err := toml.Decode(source, &c)
	if err != nil {
		return CLI{}, err
	}
	if keys := metadata.Undecoded(); len(keys) != 0 {
		return CLI{}, fmt.Errorf("unknown configuration key: %s", keys[0])
	}
	// Rust's unsigned config fields reject negative values before merging.
	if c.Size < 0 || c.Memsize < 0 || c.NumCores < 0 || c.NumBBs < 0 || c.NumELFs < 0 {
		return CLI{}, fmt.Errorf("counts and sizes must be nonnegative")
	}
	return c, nil
}
func (c CLI) Params() GeneratorParams {
	p := DefaultParams()
	p.Size = c.Size
	p.Memsize = c.Memsize
	p.NumCores = c.NumCores
	p.NumBBs = c.NumBBs
	p.Seed = c.Seed
	p.AuthorizePrivileges = c.AuthorizePrivileges
	return p
}
func (c CLI) Validate() error {
	if err := c.Params().Validate(); err != nil {
		return err
	}
	if c.NumELFs <= 0 {
		return fmt.Errorf("num_elfs must be greater than zero")
	}
	if c.Seed > math.MaxInt64-int64(c.NumELFs-1) {
		return fmt.Errorf("batch seed range exceeds signed 64-bit integers")
	}
	return nil
}
func (c CLI) OutputPath(index int) string {
	dir := c.OutDir
	name := filepath.Base(c.Out)
	if c.NumELFs > 1 {
		ext := filepath.Ext(name)
		if ext == name {
			ext = ""
		}
		stem := strings.TrimSuffix(name, ext)
		if ext == "" {
			ext = ".elf"
		}
		name = fmt.Sprintf("%s_%06d%s", stem, index, ext)
	}
	return filepath.Join(dir, name)
}

// Run validates before writing, generates sequential seeds, and prints throughput.
func Run(args []string, stdout, stderr io.Writer) error {
	c, err := ParseCLI(args, stdout)
	if err != nil {
		return err
	}
	if c.NumCores != 1 {
		return fmt.Errorf("only one core is supported for ELF output")
	}
	started := time.Now()
	if c.OutDir != "" || c.NumELFs > 1 {
		if err := os.MkdirAll(filepath.Dir(c.OutputPath(0)), 0755); err != nil {
			return err
		}
	}
	for i := 0; i < c.NumELFs; i++ {
		p := c.Params()
		p.Seed += int64(i)
		g, err := NewGenerator(p)
		if err != nil {
			return err
		}
		if err = g.Generate(); err != nil {
			return err
		}
		if err = g.GenELF(c.OutputPath(i)); err != nil {
			return err
		}
	}
	elapsed := time.Since(started).Seconds()
	fmt.Fprintf(stderr, "Generated %d ELF(s) in %.6fs (%.2f ELF/s)\n", c.NumELFs, elapsed, float64(c.NumELFs)/math.Max(elapsed, math.SmallestNonzeroFloat64))
	return nil
}
