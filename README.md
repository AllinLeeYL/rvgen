# rvgen

A RISC-V random program generator written in Rust.

**Status:** Early development. The CLI and randomized basic-block budget allocation
are implemented; instruction generation and ELF output are not yet implemented.
The commands below currently allocate budgets but do not write output files.

## Build

Requires Cargo and a Rust toolchain supporting the Rust 2024 edition. Mise is recommended to ease the building process.

```sh
# Use mise
mise run build 
# Or build using cargo
cargo build --release
cp target/release/rvgen .
```

## Usage

Configure a single program:

```sh
.rvgen one --num-instrs 1000 --num-cores 1 -o rvprog.elf
```

Configure a batch of programs:

```sh
.rvgen many --num-elfs 10 --num-instrs 1000 -o rvprogs
```

- `--num-instrs`: instruction budget **per core** (default: `1000`).
- `--num-cores`: cores per program (default: `1`).
- `--num-elfs`: programs in a batch, for `many` only (default: `100`).
- `-o`, `--output`: intended output file for `one` (default: `rvprog.elf`) or
  output directory for `many` (default: `rvprogs`).

The orchestrator randomly partitions each core's budget into basic blocks of
1–32 instructions. Block count is determined automatically, and block budgets
sum to the requested instruction count for each core.

Use `./target/release/rvgen --help`, `one --help`, or `many --help` to inspect the CLI.

## Layout

- `src/main.rs`: CLI entry point.
- `src/options.rs`: command options.
- `src/orchestrator.rs`: per-core workload planning and basic-block allocation.
- `src/utils.rs`: random budget partitioning.
- `src/isa/`: instruction representation, currently a placeholder.
- `rvgen-go/`: legacy code.
