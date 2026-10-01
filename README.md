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
- `--spike`: Spike executable used for the self-check (default: `spike`).
- `--no-self-check`: omit the self-check; Spike is then not needed. Implies
  `--no-entangle`.
- `--no-entangle`: omit data/control-flow entanglement (see below).
- `--guard-threshold`: also place a guard mid-block once this many
  workload-written registers are unchecked (default: `8`); `0` guards at block
  ends only.

The orchestrator randomly partitions each core's budget into basic blocks of
1–32 instructions. Block count is determined automatically, and block budgets
sum to the requested instruction count for each core.

## Code symbols

Each part of the generated code starts at an exported function symbol, so it
is labeled in disassembly (`objdump -d`) and usable as a Spike `until pc`
target:

| Symbol | Contents |
|---|---|
| `_start` | entry point; points `mtvec` at `_trap_handler` |
| `_init` | randomizes CSRs, FP registers, then x1-x31 |
| `_stimulus` | the random workload (all basic blocks) |
| `_check` | the self-check (omitted with `--no-self-check`) |
| `_exit` | reports the verdict to `tohost` and spins |
| `_fail` | reports a control-flow divergence to `tohost` and spins (omitted with `--no-entangle`) |
| `_trap_handler` | reports `mcause + 1` to `tohost` and spins |

## Self-checking programs

By default every program checks its own result, so a device under test only
needs to observe one `tohost` write: no register dump, commit log, or custom
testbench. Generation runs in two passes:

1. Generate the program with a `_check` block whose expected values are
   placeholders. The block loads every constant with a fixed-length sequence,
   so its size does not depend on the values.
2. Run Spike until `_check` and read x1-x31 (and f0-f31 with F), stopping
   at each entanglement site on the way to read its registers.
3. Patch those values into the sites and `_check`. No code moves, so the
   state reaching each of them is unchanged.
4. Run the final program on Spike and require it to pass.

`_check` XORs each register with its expected value and ORs the differences
into one accumulator. Since the workload may use every register, x31 is
stashed in `mscratch` as the first scratch register, x1 becomes the
accumulator once checked, and checked registers are reused afterwards. FP
registers are compared through `fmv.x.d` (RV64D) or `fmv.x.w` (low 32 bits
otherwise). The verdict is computed without branches and reported to `tohost`
as an HTIF exit code:

| Exit code | Meaning |
|---|---|
| 0 | all registers match Spike |
| 170 (`0xaa`) | at least one register mismatches |
| 171 (`0xab`) | an entangled branch or jump went the wrong way |
| `mcause + 1` | the program trapped |

Memory contents are not checked. Only core 0's program is packaged and
checked.

## Data/control-flow entanglement

The final check only sees values that survive until `_check`, and the workload
overwrites most of them long before. With entanglement (the default), the
workload's integer results also steer the program while it runs, after
[Cascade](https://comsec.ethz.ch/cascade): a core that miscomputes a value
diverges within a few instructions instead of carrying on silently. Generation
inserts sites (`src/entangle.rs`) with placeholder constants, and one Spike
run reports the registers at every site in program order; the constants are
then patched in place like `_check`'s.

| Site | Code | Catches a wrong value by |
|---|---|---|
| address | `rb = K; rb ^= r1; ...; rb ^= rk; ld x, imm(rb)` | accessing a wrong address (often a trap) |
| branch | `b<cc> r1, r2, +8; jal _fail` (`cc` chosen from Spike's values) | going the wrong way |
| indirect jump | `rt = K; rt ^= r1; ...; jalr rd, rt; jal _fail` | jumping to a wrong target |
| guard | `acc = K; acc ^= r1; ...; acc ^= rk; beq/bne acc, x0; jal _fail` | any difference, exactly |

Every memory access gets an address site (replacing the plain base load), and
every block ends with an optional branch or indirect jump (50%) and a guard.
`K` is patched to the target XORed with the `ri`'s expected values (0 for a
guard), and branch opcodes are picked so the planned direction holds. Sites
use *fresh* registers, those the workload wrote since the last guard: address,
branch and jump sites peek at the freshest few, and guards consume them all.

Measured with `scripts/inject_faults.py` (flip one workload `add`/`sub`/
`addi`/`xori`/`ori`, 200 mutants over 50 RV64IM programs of 1000
instructions), as a share of mutants that actually change a value:

| Configuration | Faults caught | Dynamic instructions |
|---|---|---|
| `--no-entangle` | 2% | ~2300 |
| `--guard-threshold 0` | 71% | ~4900 |
| `--guard-threshold 8` (default) | 81% | ~5400 |
| `--guard-threshold 4` | 91% | ~6500 |

Caught faults end the program a median of 14–25 instructions after the fault.
Faults are missed when the workload overwrites the register before a guard
reaches it. Writes by compressed instructions and FP registers are not
tracked by sites; `_check` still covers what survives to the end. Spike runs with `--isa` derived from `--xlen`/`--isa` and with
`-m<ram-base>:<ram-size>`.

Use `./target/release/rvgen --help`, `one --help`, or `many --help` to inspect the CLI.

## Layout

- `src/main.rs`: CLI entry point.
- `src/options.rs`: command options.
- `src/orchestrator.rs`: per-core workload planning and basic-block allocation.
- `src/target.rs`: fixed ISA configuration and instruction selection policy.
- `src/memory.rs`: allocated sections and memory access bounds.
- `src/elf.rs`: ELF serialization from basic blocks, memory layout, and target.
- `src/hart.rs`: per-hart init, self-check, exit, fail, and trap-handler code.
- `src/entangle.rs`: data/control-flow entanglement sites.
- `src/spike.rs`: Spike runs that compute and verify the self-check's and
  sites' values.
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
