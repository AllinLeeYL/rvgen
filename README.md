# rvgen

A self-contained Rust package with a library and an `rvgen` executable. Building
and running it requires neither Python nor a RISC-V toolchain.

```sh
cargo build --release --locked
cargo run --release --locked -- --size 256 --seed 42 -o output.elf
cargo run -- --help
```

The executable emits a bare-metal ELF with `.text` loaded at its entry address
(`0x80000000` by default). The runtime installs a machine-mode trap handler and
writes `1` to `tohost` on normal completion. Traps write `3`, reporting failure
instead of hanging. Writable, 64-byte-aligned `.tohost` and `.fromhost` sections
contain 8-byte mailboxes; `.symtab` exposes the symbols Spike discovers. This
works for RV32 and RV64.

```sh
spike -m64 output.elf
```

`generate()` now fills blocks with concrete instructions. The seed controls both
weighted choices and their operands, so different seeds produce different ELF
workloads. The default policy includes memory accesses and deliberate exceptions;
these may reach the runtime's failure handler. `--size 0` emits only the runtime.

## Repository layout

```text
src/
  lib.rs                  Public library API
  main.rs, cli.rs         Executable and CLI/config parsing
  generator/
    mod.rs                Generator orchestration and output
    params.rs             Generator/core/block parameters and validation
    program.rs            Core/block budgets, lowering, and selection records
    policy.rs             Instruction/action choices and weights
    lowering.rs           Operand generation and action sequences
  riscv/
    instruction.rs        Instruction table, kinds/classes, and encoding dispatch
    encoding/             Bit-packing helpers grouped by ISA extension
    registers.rs, ...     Registers and architectural metadata
  elf.rs                  ELF sections, symbols, and file layout
  runtime.rs              Bare-metal entry, trap handler, and HTIF exit
examples/                 Explicit instruction program and CLI config
templates/                Embedded assembly runtime template
tests/                    Integration tests
docs/legacy/              Historical configuration, not loaded by Rust
```

## Generation policy

`Instruction` is a concrete ISA opcode with fully specified operands.
`InstructionKind` is its operand-free identity, and `InstructionClass` groups
kinds such as `Alu`, `Memory`, `Amo`, and `Float` for weighted selection. Kinds,
class membership, operand construction, and encoding dispatch come from the same
instruction macro/table; there is no second opcode list in the generator.

`GenerationAction` contains high-level behavior only. `GenerationChoice` is
`Instruction(InstructionClass)` or `Action(GenerationAction)`. The lowering path is:

```text
GenerationChoice
  -> InstructionClass -> select InstructionKind -> generate typed operands
  -> GenerationAction -> generate operands for a complete sequence
  -> Vec<Instruction> -> block.insts -> encoding / assembly / ELF
```

`RegFsm` materializes a random signed 32-bit register value in two instructions;
`CreateAddressDependency` emits a register copy followed by a load using that
copy as its address. `Exception` emits ECALL or EBREAK. `FreePolluted` emits
nothing because the current operand generator has no register-pollution
bookkeeping to release. Other actions, including IPIs, privilege transitions,
and self-modifying-code operations, require a future platform/state model.
Enabling them returns an explicit error before sampling. Their default weights
are zero, including for multiple cores.

[generator/policy.rs](src/generator/policy.rs) defines stable default weights.
`Generator::choice_weights` holds per-instance overrides. Blocks record
`selected_choices`, each with the range of instructions it emitted, including
empty ranges for generator-only actions. Repeating `generate()` rebuilds the
layout and workload from current parameters; failed generation retains the
previous workload.

```rust
use rvgen::{Generator, GeneratorParams, InstructionClass};
use rvgen::generator::{GenerationAction, GenerationChoice};

let mut generator = Generator::new(GeneratorParams::default()).unwrap();
generator.choice_weights = vec![
    (GenerationChoice::Instruction(InstructionClass::Alu), 1.0),
    (GenerationChoice::Action(GenerationAction::RegFsm), 0.1),
];
generator.generate().unwrap();
assert_eq!(generator.cores[0].bbs.iter().map(|bb| bb.insts.len()).sum::<usize>(), 256);
```

The initial operand policy uses caller-saved integer/FP registers and encodable
immediates, shift amounts, rounding modes, and atomic ordering bits. Branches and
JAL target the following instruction. Memory bases and indirect jumps are not
resolved against a platform memory map, so generated programs are not guaranteed
to complete successfully. FP and atomic classes are opt-in and require their ISA
extensions; enabling FP execution also requires appropriate machine state.
Compressed instructions remain available for explicit construction. RV32 defaults
disable RV64 families, and explicitly enabling one on RV32 is an error. Disabling
`authorize_privileges` rejects the CSR class; it does not suppress ECALL/EBREAK.

Library migration:

| Previous API | Current API |
| --- | --- |
| `GenerationAction::Alu` (and other ISA families) | `GenerationChoice::Instruction(InstructionClass::Alu)` |
| `GenerationAction::RandomCsr` | `GenerationChoice::Instruction(InstructionClass::Csr)` |
| `GenerationAction::SendIpi` (and other behaviors) in weights | `GenerationChoice::Action(GenerationAction::SendIpi)` |
| `policy::DEFAULT_ACTION_WEIGHTS` | `policy::DEFAULT_CHOICE_WEIGHTS` |
| `Generator::action_weights` | `Generator::choice_weights` |
| `BasicBlockGenerator::selected_actions` | `BasicBlockGenerator::selected_choices` with emitted instruction ranges |
| Block/core `generate(rng, weights)` | `generate(rng, weights, &GenerationContext)` |

Existing low-level encoder paths under `rvgen::riscv` remain available through
re-exports; their implementations now live under `riscv::encoding`.

## Instruction representation

[`Instruction`](src/riscv/instruction.rs) is an enum with named operands for all 196
instruction encoders. Integer and floating-point registers use distinct `IntReg`
and `FloatReg` types. A block stores `Vec<Instruction>`; encoding happens when
emitting bytecode or assembly, so operand mutations cannot leave cached bytes
out of date.

```rust
use rvgen::{Generator, GeneratorParams, Instruction};
use rvgen::riscv::IntReg;

let mut instruction = Instruction::Addi {
    rd: IntReg::a0,
    rs1: IntReg::zero,
    imm: 6,
};
if let Instruction::Addi { imm, .. } = &mut instruction {
    *imm = 42;
}
assert_eq!(instruction.encode().bits(), 0x02a00513);
assert_eq!(instruction.byte_len(), 4);

let mut generator = Generator::new(GeneratorParams::default()).unwrap();
generator.cores[0].bbs[0].insts.push(instruction);
generator.gen_elf("answer.elf").unwrap();
```

This keeps three responsibilities distinct:

- `Instruction` stores the opcode and its operands. Its `encode()` method
  dispatches to the pure bit-packing functions under `riscv::encoding`.
- `EncodedInstruction` represents a 32-bit word or 16-bit halfword and writes
  little-endian bytes. Assembly rendering uses `.4byte`/`.2byte` directives to
  preserve the same bits and widths without assembler relaxation.
- Fuzzer-specific state, such as hart IDs, event IDs, observed addresses, and
  coverage, belongs in the generator or a separate record that owns an
  `Instruction`. Encoding does not depend on simulation state.

An enum lets mutation code match instruction kinds and edit their actual
operands, without a class hierarchy, heap allocation per instruction, or string
opcode dispatch. A single declaration table connects each variant to its
encoder, width, canonical mnemonic, and optional sampling class. `kind()`,
`class()`, `mnemonic()`, and
`syntactic_dependency()` expose existing static metadata; they do not construct
a dynamic dependency graph or check RVWMO executions.

`Instruction::Standard(u32)` and `Instruction::Compressed(u16)` retain the raw
instruction API for custom opcodes and deliberate illegal-instruction tests.
`rvgen::generator::Instruction` remains available as a re-export.

The low-level encoders retain their existing names, argument order, immediate
masking, and assertions. They cover RV32/RV64 I, M, A, F, D, C, Zicsr, Zifencei,
and privileged instructions. Their arguments are `i32`, except atomic ordering
booleans, and their result is `u32`. Branch/jump offsets are bytes; LUI/AUIPC
immediates are the upper 20-bit field. The operand-bearing API fixes LR's unused
`rs2` field to zero.

This refactor does not provide full ISA/XLEN validation or repair the existing
compressed-helper bit-layout quirks. The new API checks that compressed results
fit 16 bits and panics instead of silently truncating. Callers remain responsible
for valid immediates, compressed register subsets, and enabled extensions.

For an explicit instruction program, see [examples/encode.rs](examples/encode.rs):

```sh
cargo run --example encode -- encoded.elf
spike -m64 encoded.elf
```

## Command-line configuration

| Option | Default | Meaning |
| --- | --- | --- |
| `--config PATH` | none | Read top-level TOML defaults |
| `--size N` | 256 | Total emitted workload instructions across all cores and blocks |
| `--memsize N` | 4096 | Reserved parameter; currently unused |
| `--num-cores N` | 1 | Core count; ELF output requires exactly one |
| `--num-bbs N` | 12 | Basic blocks per core |
| `--seed N` | 0 | Seed for reproducible choices and operands |
| `--authorize-privileges VALUE` | true | Allow privileged generation, including the opt-in CSR class |
| `--out PATH`, `-o PATH` | output.elf | ELF destination |
| `--num-elfs N` | 1 | Generate N files in one process, with consecutive seeds |
| `--out-dir PATH` | parent of --out | Output directory, created when supplied or in batch mode |

Core, block, and ELF counts must be positive; size may be zero. `size` counts
emitted workload instructions, excluding runtime entry/exit code. It is divided
across cores, then blocks, with remainders assigned to earlier cores/blocks.
`CoreGeneratorParams::num_insts` is the total per core;
`BasicBlockGeneratorParams::num_insts` is the budget for one block. For example,
`size = 21`, two cores, and three blocks gives block budgets `[4, 4, 3]` and
`[4, 3, 3]`. Empty blocks are allowed when there are fewer instructions than blocks.

An action sequence consumes its full instruction count and never crosses a block
boundary. Choices too large for the remaining budget are excluded and another
choice is sampled. If no enabled choice can fill the budget, generation returns
an error instead of truncating a sequence or exceeding the requested size. A
zero-output choice can run once between instruction emissions; a policy consisting
only of zero-output actions errors for a nonempty budget instead of looping.

Only `1`, `true`, and `yes` (case insensitive) enable a boolean option. Repeating
`generate()` with the same parameters and seed reproduces choices, operands, and
bytes; `Cargo.lock` pins the RNG dependencies. Signed seeds retain the existing
absolute-value convention. The previous scaffold's selection stream changes
because generating operands now consumes randomness as well.

`--config` accepts the keys in [examples/config.toml](examples/config.toml), including
`num_elfs`. Explicit CLI values take precedence. Unknown keys and incorrect TOML
value types are errors. Paths are relative to the working directory.
Historical weight tables and Chipyard settings are archived in
[docs/legacy](docs/legacy/README.md); the Rust CLI does not load them.

```sh
cargo run --release --locked -- --config examples/config.toml -o configured.elf
cargo run --release --locked -- --num-elfs 1000 --out-dir generated -o case.elf
```

This creates `case_000000.elf` through `case_000999.elf`. With one ELF the original
filename is retained. Existing destination files are overwritten. Each command
prints generation/write time and ELF/s; parsing and startup are outside that
timer. This measures workload generation and ELF writing throughput.

The library's `GeneratorParams` also exposes `is_64bit` and `start_addr` for ELF32
and custom entry addresses. `ElfBuilder` accepts explicit section addresses,
bytes, flags, and alignment. Callers must choose addresses consistent with the
entry point and load-segment alignment. Section names must be unique ASCII names
without NULs; alignment must be zero, one, or a power of two.

The assembly template is embedded and works from any working directory. Its
external `workload` and `__stack_top` references must be supplied when linking
assembly; ELF generation uses the separate built-in runtime.

## Validation

```sh
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
```

Tests check instruction encodings against independent reference words verified
with GNU RISC-V tools, operand mutation, atomic ordering, mixed-width emission,
ELF parsing, generator behavior, and CLI integration. Python parity tests and
frozen Python snapshots have been removed. Neither Python nor GNU tools are test
dependencies. Runtime tests also execute RV32/RV64 success and trap cases in
Spike when it is installed; set `SPIKE` to override its executable path.

Ported RISC-V source retains its original copyright notices. See [LICENSE](LICENSE)
for GPL-3.0-only terms.
