# rvgen

A RISC-V random program generator written in Rust.

**Status:** Early development. Generates randomized RISC-V basic blocks and
serializes them with a resolved memory layout as ELF32 or ELF64.

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
./rvgen one --num-instrs 1000 --num-cores 1 -o rvprog.elf
```

Configure a batch of programs:

```sh
./rvgen many --num-elfs 10 --num-instrs 1000 -o rvprogs
```

- `--num-instrs`: instruction budget **per core** (default: `1000`).
- `--num-cores`: cores per program (default: `1`).
- `--num-elfs`: programs in a batch, for `many` only (default: `100`).
- `--xlen`: target register width, `32` or `64` (default: `64`).
- `--isa`: comma-separated extensions (default: `i,zicsr`); Zicsr is required for the trap handler, and D implies F.
- `--ram-base`, `--ram-size`: allocation bounds (default: `0x80000000`, `0x08000000`).
- `--scratch-size`, `--smc-size`: section sizes in bytes (default: `4096` each); zero SMC size omits it.
- `-o`, `--output`: output file for `one` (default: `rvprog.elf`) or
  output directory for `many` (default: `rvprogs`).

The orchestrator randomly partitions each core's budget into basic blocks of
1–32 instructions. Block count is determined automatically, and block budgets
sum to the requested instruction count for each core.

Use `./target/release/rvgen --help`, `one --help`, or `many --help` to inspect the CLI.

## Layout

- `src/main.rs`: CLI entry point.
- `src/options.rs`: command options.
- `src/orchestrator.rs`: per-core workload planning and basic-block allocation.
- `src/target.rs`: fixed ISA configuration and instruction selection policy.
- `src/memory.rs`: allocated sections and memory access bounds.
- `src/elf.rs`: ELF serialization from basic blocks, memory layout, and target.
- `src/riscv/`: instruction representation and encoding.
- `src/utils.rs`: random budget partitioning.

`Orchestrator` owns the fixed target, per-core basic blocks, shared mutable state,
and RNG. The CLI reserves 8-byte `.tohost` and `.fromhost` sections aligned to
64 bytes, scratch, and optional SMC memory at the top of RAM and places core 0's generated code at the RAM base. `Elf::new(&target).encode(bbs,
&memory)` preserves the supplied instruction order, section addresses, sizes,
alignment, and permissions. Other supplied sections are zero-filled. The ELF
encoder exports `tohost` and `fromhost` symbols for their supplied sections.

Startup, register initialization (including the scratch base in `gp`), trap
handling, and termination must be supplied as instructions by generation. The
ELF encoder adds no code and requires no scratch or HTIF sections.
