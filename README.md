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
- `--priv`: comma-separated privilege modes `m`, `s`, `u` (default: `m,s,u`); M is required and S requires U. Without S, no S-mode CSR such as `sscratch` is accessed.
- `--ram-base`, `--ram-size`: allocation bounds (default: `0x80000000`, `0x08000000`).
- `--scratch-size`, `--smc-size`: section sizes in bytes (default: `8192` each); zero SMC size omits it.
- `-o`, `--output`: output file for `one` (default: `rvprog.elf`) or
  output directory for `many` (default: `rvprogs`).
- `--spike`: Spike executable used for the self-check (default: `spike`).
- `--no-self-check`: omit the self-check; Spike is then not needed. Implies
  `--no-entangle`.
- `--no-entangle`: omit data/control-flow entanglement (see below).
- `--guard-threshold`: also place a guard mid-block once this many
  workload-written registers are unchecked (default: `8`); `0` guards at block
  ends only.
- `--inline-golden`: build the code's constants (sites' golden values,
  `_init`'s values, `_check`'s expected values) inline instead of loading them
  from `.golden` (see below). RV32 always does this.
- `--weight OPCODE=W`, `--class-weight CLASS=W`, `--default-weight W`: the
  instruction mix, see [Instruction weights](#instruction-weights).
- `--seed`: makes generation reproducible (default: random). `many` uses
  `seed + index` for each program.

The orchestrator randomly partitions each core's budget into basic blocks of
1–32 instructions. Block count is determined automatically, and block budgets
sum to the requested instruction count for each core.

## Instruction weights

Every instruction in the workload is drawn from a weighted distribution over
the opcodes the target supports. By default all opcodes are equally likely.
Weights are relative (no need to sum to 1) and are given on the command line:

```sh
# 4x as many loads/stores as anything else, no multiplies, extra `xor`s
./rvgen one --isa i,m,zicsr --class-weight memory=4 --class-weight muldiv=0 \
    --weight xor=10 --seed 1
# comma-separated also works
./rvgen one --weight add=3,sub=3,mulh=0
```

- `--weight MNEMONIC=W` sets one opcode (`add`, `c.addi`, `fadd.s`, ...).
- `--class-weight CLASS=W` sets every opcode of a class. Classes (case
  insensitive): `Other`, `Alu`, `Alu64`, `MulDiv`, `MulDiv64`, `Memory`,
  `Memory64`, `Branch`, `Jal`, `Jalr`, `Amo`, `Amo64`, `FloatMemory`, `Float`,
  `Float64`, `DoubleMemory`, `Double`, `Double64`, `Fence`, `Csr`.
- `--default-weight W` (default `1`) applies to everything not named.
- Precedence: an opcode weight beats a class weight beats the default,
  whatever the order of the flags.
- A weight of 0 excludes the opcode. Weights must be finite and not negative.
  A nonzero weight that could never take effect is an error, not a silent
  no-op: an opcode the ISA/XLEN does not support, one in `--disabled-instrs`,
  one the generator never emits (`jal`, `ecall`, `ebreak`, `mret`, `sret`,
  compressed jumps and branches), or a branch without entanglement.
- The weights count *workload* instructions. The code the generator adds
  around them (address computation, guards, the self-check, and so on) is not
  weighted and always uses its own instructions, such as `xor`, `beq`, `bne`
  and `jal`. A weight of 0 for `beq` therefore removes `beq` from the
  workload, not from the guards.

**Branch frequency.** The six conditional branches and `jalr` are drawn like
any other opcode, so their share of the total weight is their density: raise
`--class-weight branch=` or `--weight jalr=` for more control flow, set it to 0
for none. Each one becomes an entanglement site (see below), which needs the
self-check; setting it without entanglement is an error. Unless named, these
seven opcodes get a quarter of the default weight each, close to the density
before they were weighted. `Branch` and `Jalr` are separate classes. The
branch weights add up to the density, but which of the six a site uses also
depends on the values Spike observed, since the opcode must go the planned
way: among the opcodes that do, the weights choose. Weighting a single branch
opcode therefore biases the mix without making it exclusive.

The weights are plain data, so another program can build the command line (the
full weight vector is a few kilobytes) or call the library.

## Library

`rvgen` is also a Rust library: the binary is a thin wrapper. Fill in
`CommonOpts` (its `Default` is the CLI's defaults) and call `generate`, which
returns the ELF image without touching the file system:

```rust
use rvgen::{options::CommonOpts, orchestrator::generate, riscv::{InstructionClass, Opcode}};

let mut opts = CommonOpts::default();
opts.seed = Some(42);
opts.class_weights.push((InstructionClass::Branch, 4.0));
opts.opcode_weights.push((Opcode::Jalr, 0.0));
let elf: Vec<u8> = generate(&opts)?;
```

Spike is still needed for self-checking programs. For finer control over the
mix, e.g. changing it along the program, implement `weights::WeightPolicy`:
the orchestrator asks it for each basic block's weights.

## Code symbols

Each part of the generated code starts at an exported function symbol, so it
is labeled in disassembly (`objdump -d`) and usable as a Spike `until pc`
target:

| Symbol | Contents |
|---|---|
| `_start` | entry point; points `mtvec` at `_trap_handler` |
| `_init` | randomizes CSRs, FP registers, then x1-x31, the memory base registers getting their addresses |
| `_stimulus` | the random workload (all basic blocks) |
| `_check` | the self-check (omitted with `--no-self-check`) |
| `_exit` | reports the verdict to `tohost` and spins |
| `_fail` | reports a control-flow divergence to `tohost` and spins (omitted with `--no-entangle`) |
| `_trap_handler` | reports `mcause + 1` to `tohost` and spins |

The `.golden` data section, exported as the `_golden` object symbol, holds the
constants the code loads (RV64 only): the entanglement sites' golden values,
then `_init`'s values and `_check`'s expected values.

## Memory accesses

Each program reserves 3–5 registers, drawn from `gp`, `tp` and `s2`–`s11`, as
memory bases (`src/membase.rs`). `_init` points each one at a random
doubleword in a data section (`scratch`), and nothing writes them afterwards:
the workload never samples them as a destination, and generator code avoids
them. Every memory access reaches a random aligned address in the section
through one of the bases:

| Access | Code |
|---|---|
| 12-bit immediate (`lb`…`sd`, `flw`…`fsd`) | the access, rewritten to `imm(base)` |
| short or no immediate (compressed, `*sp`, AMO) | `addi rs1, base, off`; the access |
| data-dependent | `andi rs1, r, 0x7f8`; `add rs1, rs1, base`; the access |

A data-dependent access mixes in the most recently written workload register
`r`, masked so the address stays in the section whatever `r` holds. Each
program draws its share of data-dependent accesses (among those with a 12-bit
immediate) uniformly from 0–10%.

## Self-checking programs

By default every program checks its own result, so a device under test only
needs to observe one `tohost` write: no register dump, commit log, or custom
testbench. Generation runs in two passes:

1. Generate the program with a `_check` block whose expected values are
   placeholders. The block loads every constant from `.golden`, or inline
   with a fixed-length sequence, so its size does not depend on the values.
2. Run Spike until `_check` and read x1-x31 (and f0-f31 with F), stopping
   at each entanglement site on the way to read its registers.
3. Patch those values into the sites and `_check` (into `.golden` on RV64).
   No code moves, so the state reaching each of them is unchanged.
4. Run the final program on Spike and require it to pass.

`_check` XORs each register with its expected value and ORs the differences
into one accumulator. Since the workload may use every register, x31 is
stashed in `mscratch` as the first scratch register, x1 becomes the
accumulator once checked, and checked registers are reused afterwards; from
`.golden`, x2 then points at the expected values, so each register costs an
`ld`, an `xor` and an `or`. FP
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
then patched in place like `_check`'s. The constant a value site starts from
is its *golden value*, decided by Spike, the golden model.

| Site | Code | Catches a wrong value by |
|---|---|---|
| branch | `b<cc> r1, r2, +8; jal _fail` (`cc` chosen from Spike's values) | going the wrong way |
| indirect jump | `rt = golden; rt ^= r1; ...; jalr rd, rt; jal _fail` | jumping to a wrong target |
| guard | `acc = golden; acc ^= r1; ...; acc ^= rk; beq/bne acc, x0; jal _fail` | any difference, exactly |

Every block ends with an optional branch or indirect jump (50%) and a guard.
Every memory access gets an address site (replacing the plain base load),
and the workload's weighted draws of branches and `jalr` become branch and
jump sites wherever they fall in a block (see
[Instruction weights](#instruction-weights)).
The golden value is the target XORed with the `ri`'s expected values (0 for a
guard), and branch opcodes are picked so the planned direction holds. Sites
use *fresh* registers, those the workload wrote since the last guard: branch
and jump sites peek at the freshest few, and guards consume them all. Memory
accesses are not sites (see [Memory accesses](#memory-accesses)); a
data-dependent address is fresh, so the next guard checks it.

On RV64, `x = golden` is `auipc x, %hi(slot); ld x, %lo(slot)(x)`: each value
site reads its own 8-byte slot of `.golden`, a section reserved below the
workload's data and never targeted by its accesses. Only the slot is patched
after the Spike run; the two instructions are fixed once the code is linked.
`_init` and `_check` likewise point a register at their own slots with
`auipc; addi` and load one constant per register. Built inline instead
(`--inline-golden`, and always on RV32), a golden or expected value takes
`load_imm_fixed`'s 8 instructions on RV64 (2 on RV32). For 100 default
programs (seeds 1000–1099), loading cuts the dynamic length from 3404 to 2272
instructions. With only the sites' values loaded, `scripts/inject_faults.py`
caught the same faults (79.1% vs 79.2% of 300 paired mutants). The table below
predates `.golden`.

Measured with `scripts/inject_faults.py` (flip one `add`/`sub`/`addi`/`xori`/
`ori` in `_stimulus`, 200 mutants over 50 RV64IM programs of 1000
instructions), as a share of mutants that actually change a value:

| Configuration | Faults caught | Dynamic instructions |
|---|---|---|
| `--no-entangle` | 17% | ~1600 |
| `--guard-threshold 0` | 65% | ~2900 |
| `--guard-threshold 8` (default) | 82% | ~3600 |
| `--guard-threshold 4` | 91% | ~4800 |

The mutated instruction may be a base `addi` in front of an access; those
mutants trap at once and account for most of the catches without
entanglement. Faults caught by a site end the program a median of 15–28
instructions after the fault.
Faults are missed when the workload overwrites the register before a guard
reaches it. Writes by compressed instructions and FP registers are not
tracked by sites; `_check` still covers what survives to the end. Spike runs with `--isa` derived from `--xlen`/`--isa`, `--priv` from `--priv`, and with
`-m<ram-base>:<ram-size>`.

Use `./target/release/rvgen --help`, `one --help`, or `many --help` to inspect the CLI.

## Layout

- `src/main.rs`: CLI entry point.
- `src/lib.rs`: the library the CLI wraps.
- `src/options.rs`: command options.
- `src/weights.rs`: instruction weights, their validation, and per-block policy.
- `src/orchestrator.rs`: per-core workload planning and basic-block allocation.
- `src/target.rs`: fixed ISA configuration and instruction selection policy.
- `src/memory.rs`: allocated sections and memory access bounds.
- `src/elf.rs`: ELF serialization from basic blocks, memory layout, and target.
- `src/hart.rs`: per-hart init, self-check, exit, fail, and trap-handler code.
- `src/entangle.rs`: data/control-flow entanglement sites.
- `src/membase.rs`: reserved memory base registers and access placement.
- `src/spike.rs`: Spike runs that compute and verify the self-check's and
  sites' values.
- `src/riscv/`: instruction representation and encoding.
- `src/utils.rs`: random budget partitioning.

`Orchestrator` owns the fixed target, per-core basic blocks, shared mutable state,
and RNG. The CLI reserves 8-byte `.tohost` and `.fromhost` sections aligned to
64 bytes, scratch, and optional SMC memory at the top of RAM, then `.golden`
below them once generation knows how many constants the code loads, and
places core 0's generated code at the RAM base. `Elf::new(&target).encode(bbs,
&memory)` preserves the supplied instruction order, section addresses, sizes,
alignment, and permissions. Other supplied sections are zero-filled until
filled with `Elf::fill` (as `.golden` is). The ELF
encoder exports `tohost` and `fromhost` symbols for their supplied sections.

Startup, register initialization (including the memory base registers), trap
handling, and termination must be supplied as instructions by generation. The
ELF encoder adds no code and requires no scratch or HTIF sections.
