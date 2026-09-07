# rvgen (Rust)

Native Rust port of the Python project in `../src/rvgen`. This directory is a
self-contained Cargo package with a library and an `rvgen` executable. Python,
Jinja2, Z3, and a RISC-V toolchain are not needed to build or run it.

```sh
cd rvgen
cargo build --release --locked
cargo run --release --locked -- --size 256 --seed 42 -o output.elf
cargo run -- --help
```

Both generators now emit a bare-metal executable with `.text` loaded at its
entry address (`0x80000000` by default). The runtime installs a machine-mode trap
handler and writes `1` to the `tohost` mailbox on normal completion. Traps write
`3`, reporting failure instead of hanging. Writable, 64-byte-aligned `.tohost`
and `.fromhost` sections contain 8-byte mailboxes; `.symtab` exposes the `tohost`
and `fromhost` symbols that Spike discovers. This works for RV32 and RV64.

```sh
spike -m64 output.elf
```

Random workload construction remains a scaffold: `generate()` samples classes
but does not yet construct random instructions. The default ELF therefore runs
the entry/exit runtime with an empty workload and stops successfully. Different
seeds affect class selection, but currently produce identical ELF payloads.

The instruction encoders and ELF writer are usable independently. For an ELF
containing explicitly encoded instructions with its entry inside `.text`, run:

```sh
cargo run --example encode -- encoded.elf
```

Command-line options and their defaults:

| Option | Default | Meaning |
| --- | --- | --- |
| `--config PATH` | none | Read top-level TOML defaults |
| `--size N` | 256 | Class selections **per basic block**, divided by core count |
| `--memsize N` | 4096 | Reserved parameter; currently unused |
| `--num-cores N` | 1 | Core count; ELF output requires exactly one |
| `--num-bbs N` | 12 | Basic blocks per core |
| `--seed N` | 0 | Seed for reproducible class selection |
| `--authorize-privileges VALUE` | true | Reserved parameter; currently unused |
| `--out PATH`, `-o PATH` | output.elf | ELF destination |
| `--count N` | 1 | Generate N files in a single process, using seeds seed through seed+N-1 |
| `--out-dir PATH` | parent of --out | Output directory; created when supplied or in batch mode |

Core and block counts must be positive; size may be zero. Like the Python CLI,
only `1`, `true`, and `yes` (case insensitive) enable a boolean option.
`size / num_cores` uses integer division, retaining the original generator's
per-block sizing semantics.

`--config` accepts the keys shown in [example.toml](example.toml), using
underscores for `num_cores`, `num_bbs`, and `authorize_privileges`. Explicit CLI
values take precedence. Unknown keys and wrong TOML value types are errors.
Paths are relative to the working directory, including paths in the config.
The parent directory's `rvgen.toml` contains fuzzer weight tables and is not a
CLI configuration file; the Python CLI also rejects it.

For multiple files, `-o case.elf --count 3` creates `case_000000.elf` through
`case_000002.elf`. `--out-dir` selects the directory and `--out` supplies the
basename. With count 1 the original filename is retained. Existing destination
files are overwritten, as with single-file output. Each command prints elapsed
generation/write time and ELF/s; parsing and startup are outside that timer.

From the repository root, compare the same batch in each implementation:

```sh
cargo build --release --locked --manifest-path rvgen/Cargo.toml
rvgen/target/release/rvgen --count 1000 --size 256 --num-bbs 12 --seed 0 --out-dir elfs/rust
PYTHONPATH=src python3 -m rvgen.cli --count 1000 --size 256 --num-bbs 12 --seed 0 --out-dir elfs/python
python3 scripts/benchmark.py --count 1000 --repeats 3
```

The benchmark helper uses temporary directories, warms each executable, alternates
run order, and reports median wall time including startup and file writes.
Compilation, simulation, and output cleanup are excluded. `--temp-dir` chooses
the filesystem to measure. Because random bodies are still unimplemented, these
numbers compare class selection, runtime ELF construction, and file writing.

The library preserves the original module and encoder function names under
`rvgen::riscv`, including RV32/RV64 I, M, A, F, D, C, Zicsr, Zifencei, and
privileged instructions. Encoder arguments are `i32` (except atomic ordering
booleans) and encoded words are `u32`. Compressed encoders also return `u32` to
preserve the Python helpers' raw bitfield results. Use `u16::try_from` when
storing a compressed instruction. These functions retain the original immediate
masking and bounds checks; they do not validate every architectural operand rule.
Encoder outputs, including existing compressed-helper bit-layout quirks, are
preserved. The compatibility tests are not an ISA-conformance audit.

Other ported components include register names, CSR identifiers and aliases,
architectural constants, assembly helpers, the RVWMO dependency table, weighted
class selection, the generator/core/block hierarchy, assembly rendering, and the
ELF32/ELF64 writer. RVWMO's references to absent Python instruction classes are
represented by `MemoryInstructionKind` variants; it remains metadata rather than
a memory-model checker.

Differences from the Python scaffold:

- All parsed generator options reach `GeneratorParams` in both implementations;
  the original Python entry point only forwarded `size`.
- Class selections are retained in `selected_classes`, and generation uses the
  seed. Repeating `generate()` reproduces those selections. Rust's RNG does not
  reproduce Python's random sequence; `Cargo.lock` pins its dependencies.
- Weights belong to each generator in both implementations, avoiding global mutation.
- `get_bytecode()` serializes instructions explicitly added to a block's `insts`.
- The assembly template is embedded, so library rendering works from any working
  directory. It retains the original template's external `workload` and
  `__stack_top` references; callers must supply them when linking assembly.
- The integer register-name table has all 32 entries, fixing Python's accidentally
  concatenated `s10`, `s11`, and `t3` strings. Signed-conversion helpers use fixed
  width Rust values and check RV32 bounds.
- Invalid counts, weights, and unrepresentable ELF layouts produce errors.
  ELF section names must be unique ASCII names without NULs, and alignment must
  be zero, one, or a power of two. ELF flags retain the original defaults; callers
  building other ABI variants may need a fuller ELF/linker implementation.

The library's `GeneratorParams` also exposes `is_64bit` and `start_addr` for ELF32
and custom entry addresses. `ElfBuilder` accepts explicit section addresses,
bytes, flags, and alignment. As in Python, callers are responsible for choosing
addresses consistent with the entry point and load-segment alignment.

```sh
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
```

Tests include 6,784 frozen Python outputs spanning all 212 encoder functions,
byte-for-byte ELF32/ELF64 fixtures, assembly parity, independent ELF parsing,
seed and weight behavior, and CLI integration. Tests do not invoke Python.

Ported RISC-V source retains its original copyright notices. See [LICENSE](LICENSE)
for GPL-3.0-only terms.
