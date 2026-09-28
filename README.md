# rvgen

A RISC-V random program generator written in Rust.

**Status:** Early development. Generates randomized RISC-V workloads and wraps
them in a machine-mode runtime with HTIF termination and ELF output.

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
- `--isa`: comma-separated extensions (default: `i,zicsr`); D implies F, F implies Zicsr.
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
- `src/runtime.rs` and `src/runtime/`: shared ISA configuration, memory planning, and runtime emission.
- `src/riscv/`: instruction representation and encoding.
- `src/utils.rs`: random budget partitioning and ELF serialization.

See [runtime architecture](src/runtime/README.md) for extension-dependent startup,
memory allocation, reserved registers, and customization.
