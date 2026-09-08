# rvgen in Go

A standalone Go rewrite of the Rust library and CLI. Go compiles to native machine
code and includes a garbage collector, with no JVM or separate virtual machine.
The `go` command handles builds, dependencies, formatting, and tests. There is one
external dependency: the TOML parser, pinned in `go.mod` and verified by `go.sum`.
No Rust compiler, Python, CMake, C compiler, or RISC-V assembler is needed.

The design uses one generator, plain core/block data, an instruction table, and
functions for ELF output. See [DESIGN.md](DESIGN.md) for the data flow, invariants,
and API changes from the initial rewrite.

## Build and run

Install [Go 1.22 or newer](https://go.dev/doc/install). From the repository root:

```sh
cd go
go build -o bin/rvgen ./cmd/rvgen
./bin/rvgen --size 256 --seed 42 -o output.elf
./bin/rvgen --config examples/config.toml --num-elfs 1000 --out-dir generated
./bin/rvgen --help
```

Output defaults to `elfs/`; pass `--out-dir .` to write in the current directory.
`-o` supplies the filename; `--out-dir` supplies its directory. For example,
use `--out-dir /tmp/cases -o case.elf` to write `/tmp/cases/case.elf`.

Install the executable into a local directory on your PATH (from `go/`):

```sh
mkdir -p "$HOME/.local/bin"
GOBIN="$HOME/.local/bin" go install ./cmd/rvgen
export PATH="$HOME/.local/bin:$PATH"
rvgen --size 256 --seed 42
```

Add the PATH export to your shell configuration to keep it across sessions.

For development, run directly with `go run ./cmd/rvgen --size 256`. The first
build downloads the pinned TOML dependency; subsequent builds use Go's cache.
You can copy this directory elsewhere and build it independently.

## Debug

To debug the Go generator, install [Delve](https://github.com/go-delve/delve/blob/master/Documentation/installation/README.md).
With this workspace's Go 1.22 toolchain, use Delve v1.22.1; its
[supported Go range](https://github.com/go-delve/delve/blob/v1.22.1/pkg/goversion/compat.go)
includes Go 1.22. With a current Go toolchain, install `@latest` instead.
All commands below run from `go/`:

```sh
GOBIN="$HOME/.local/bin" go install github.com/go-delve/delve/cmd/dlv@v1.22.1
export PATH="$HOME/.local/bin:$PATH"
dlv debug ./cmd/rvgen -- --size 16 --seed 42 --out-dir /tmp/rvgen-debug
```

At the Delve prompt:

```text
break rvgen.(*Generator).Generate
continue
print g.Params
next
step
locals
stack
continue
quit
```

[`dlv debug`](https://github.com/go-delve/delve/blob/master/Documentation/usage/dlv_debug.md)
builds with optimizations disabled. To keep a separate debug binary:

```sh
go build -gcflags='all=-N -l' -o bin/rvgen-debug ./cmd/rvgen
dlv exec ./bin/rvgen-debug -- --size 16 --seed 42 --out-dir /tmp/rvgen-debug
```

To inspect or debug the generated RISC-V program, use `readelf` and
[Spike's interactive debugger](https://github.com/riscv-software-src/riscv-isa-sim#interactive-debug-mode).
Start with the explicit arithmetic example, which completes successfully:

```sh
go run ./examples/encode /tmp/rvgen-encoded.elf
readelf -h -S -s /tmp/rvgen-encoded.elf
spike -m64 /tmp/rvgen-encoded.elf
spike -d -m64 /tmp/rvgen-encoded.elf
```

At Spike's prompt, press Enter to step, `reg 0 a0` to inspect a register,
`r` to continue, or `q` to quit. Random workloads can intentionally trap;
the runtime reports traps through `tohost` as failure.

## Library example

```go
p := rvgen.DefaultParams()
p.Size = 100
p.Seed = 42

g, err := rvgen.NewGenerator(p)
if err != nil {
    return err
}
if err := g.Generate(); err != nil {
    return err
}
return g.GenELF("program.elf")
```

Instructions are ordinary structs with editable fields:

```go
inst := riscv.Instruction{
    Kind: riscv.Addi,
    Rd:   riscv.A0,
    Rs1:  riscv.Zero,
    Imm:  42,
}
word := inst.Encode().Bits // 0x02a00513
inst.Imm = 7             // Encoding always uses the current operands.
```

`Rd`, `Rs1`, `Rs2`, and `Rs3` hold register numbers. Each opcode determines whether
an operand names an integer or floating register; fields irrelevant to that
opcode are ignored. `riscv.Standard(word)` and `riscv.Compressed(halfword)` provide
raw encodings for custom or intentionally illegal instructions.

For a complete explicit workload:

```sh
go run ./examples/encode # Writes encoded.elf.
```

Pass an output filename directly, for example `go run ./examples/encode answer.elf`.

Custom generation weights remain per generator:

```go
g.ChoiceWeights = []rvgen.ChoiceWeight{
    {Choice: rvgen.InstructionChoice(riscv.ClassAlu), Weight: 1},
    {Choice: rvgen.ActionChoice(rvgen.RegFsm), Weight: 0.1},
}
```

## Behavior

The port includes all 196 instruction encoders, ISA classes, architectural
constants, CSR identifiers, and static RVWMO dependency metadata. Compressed
instructions are available for explicit construction. Encoding retains the
Rust helpers' argument order, masking, assertions, and compressed-layout quirks;
compressed encodings exceeding 16 bits panic instead of truncating.

The generator preserves weighted choices, operand generation, whole action
sequences, exact core/block budgets, selection records, and regeneration from
current parameters. Failed generation retains the previous workload. Unsupported
platform-dependent actions return errors when enabled. Memory accesses and
indirect jumps do not have a platform memory map and can trap, as in Rust.

ELF32/ELF64 output includes the entry runtime, trap handler, aligned HTIF mailboxes,
and symbols. Success writes 1 to `tohost`; traps write 3. The assembly template is
embedded in the binary. The in-memory ELF writer rejects images larger than 1 GiB
before allocating storage, including excessive alignment gaps.

The existing CLI flags, defaults, TOML keys, override precedence, numbered batch
filenames, seed-range checks, and throughput reporting are supported. `CLI` is a
single shared struct for flags and TOML; `config` itself is CLI-only. Unknown TOML
keys and incorrect types are rejected. Paths are relative to the current working
directory. Count flags accept nonnegative decimal integers, and the seed is a
signed decimal integer. The Go standard flag parser also accepts single-dash
long options, and its help/error wording differs from Clap.

**Seed compatibility:** Go uses its standard `math/rand` generator. Repeating a
seed reproduces Go's workloads, including the existing absolute-value convention
for signed seeds. The same seed does **not** produce Rust's ChaCha-based workload.
Explicit instruction encodings and the built-in runtime match Rust byte for byte.

## Performance

In a local Linux x86-64 comparison, this Go implementation produced about 1.7×
as many 256-instruction ELFs per second and 2.8× as many 4,096-instruction ELFs
as the Rust release build. See [BENCHMARKS.md](BENCHMARKS.md) for the measurements,
commands, and limits of the comparison.

## Files

| File | Purpose |
| --- | --- |
| `cli.go`, `cmd/rvgen/` | Shared CLI/TOML settings, batch loop, and executable |
| `params.go`, `policy.go`, `generator.go` | One parameter set, choices, generation, and plain workload data |
| `riscv/instruction.go`, `riscv/table.go` | Editable instructions and opcode table |
| `riscv/rv*.go`, `riscv/z*.go` | Low-level bit encoders |
| `riscv/registers.go`, `riscv/rvwmo.go` | Architectural and dependency metadata |
| `elf.go`, `runtime.go` | Executable layout and bare-metal runtime |
| `examples/` | Config and explicit instruction example |

## Validation

```sh
go test ./...
go vet ./...
go test -race ./...
go test -run '^$' -bench . -benchmem
```

Tests include 1,568 encoding fixtures covering every opcode, independent reference
words previously verified with GNU RISC-V tools, and byte-for-byte Rust ELF
runtime fixtures for RV32 and RV64. Go's `debug/elf` independently checks headers,
sections, segments, and symbols. Generator and executable tests check budgets,
seed reproducibility, class restrictions, errors, and config/batch behavior.

When `spike` is installed (or `SPIKE` names its executable), tests also execute
RV32/RV64 empty, long, compressed, illegal, and generated workloads. This test is
explicitly skipped otherwise. Fixture tests need no Rust installation; fixture
provenance is documented beside the data.

Original RISC-V source copyright notices are retained. Licensed GPL-3.0-only;
see [LICENSE](LICENSE).
