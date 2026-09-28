# Runtime and target configuration

`Runtime` replaces the old `GenContext` and standalone runtime writer. Construct
one instance before generating a program and pass it to both generation and ELF
emission. It owns the target, memory plan, and disabled workload opcodes. It does
not depend on CLI types; `orchestrator.rs` translates CLI options into these types.

| Component | Responsibility |
| --- | --- |
| `runtime.rs` / `Runtime` | Shared API, workload policy, scratch window, ELF emission |
| `target.rs` / `Target` | XLEN, normalized extension set, instruction validation |
| `memory.rs` / `MemoryPlan` | Physical RAM regions and named section requests |
| `memory.rs` / `MemoryLayout` | Resolved allocation addresses and ELF sections |
| `code.rs` / `RuntimeCode` | Machine-mode startup, HTIF exit, trap handler, address fixups |

The flow is:

1. Validate the target and memory requests.
2. Generate instructions using `runtime.workload_opcodes()` and
   `runtime.scratch_window()`. Encode them through `runtime.target().emit()`.
3. Emit startup and exit code to determine the complete `.text` size.
4. Allocate `.text` and the requested sections inside compatible RAM regions.
5. Resolve symbolic address loads and create symbols from section names.
6. Write the ELF using those same allocations.

There is no second memory map in the generator or independent XLEN argument in
the ELF writer. Configured RAM describes the target; it does not configure Spike,
a board's physical RAM, PMP, or page tables.

## ISA behavior

The CLI defaults to RV64 with `i,zicsr`. This machine-mode runtime needs Zicsr to
install `mtvec`; an explicit `--isa i` therefore produces a configuration error.
`D` implies `F`, and `F` implies `Zicsr`; duplicate extensions are deduplicated.

- Without F/D, startup emits no FP instructions and does not access `fcsr` or FS.
- F initializes all FP registers with `fmv.w.x`.
- RV64 D uses `fmv.d.x`; RV32 D uses `fcvt.d.w` to initialize all 64 bits to +0.0.
- Compressed padding is emitted only with C enabled.
- Both workload selection and runtime emission check the target ISA and XLEN.
- `--disabled-instrs` applies to randomly selected workload instructions. Runtime
  setup and memory/CSR repair instructions still use the required ISA instructions.

Extension availability does not imply that every randomly chosen CSR exists on a
particular CPU. Random CSR and privileged operations can still trap. The current
runtime enters the workload in machine mode; supervisor/user entry is not implemented.

## Memory and startup register contract

The default plan requests `.tohost`, `.fromhost`, `.scratch`, and `.smc` after
placing `.text` at the entry address. Allocation checks bounds, overlap, alignment,
permissions, and RV32 address limits. Section indices are derived from names, so
adding or reordering requests does not break symbols or startup pointers.

- `gp` (x3) points into `.scratch`. Its offset is derived from the requested size
  and supports signed 12-bit memory operands.
- `tp` (x4) points to `.smc` when that section is present.
- Both registers are reserved: the current operand sampler excludes them.
- Other integer registers start at zero. Memory setup may subsequently change
  source registers or `sp` for compressed stack-relative accesses.
- `.scratch` is writable data. Every generated memory access, including its full
  width, is constrained to that allocation.
- `.smc` is writable and executable. `--smc-size 0` omits it and its address load.
  Reserving this section does not itself generate self-modifying workloads. Code
  that writes and executes instructions must use the appropriate `fence.i` protocol
  and enable Zifencei.

The runtime exports `_start`, `_exit`, `rvgen_trap`, `tohost`, `fromhost`,
`rvgen_scratch`, and (when allocated) `rvgen_smc`. Address loads use AUIPC/ADDI;
placements outside their supported reach return an error.

## Configuration

```sh
cargo run -- one --xlen 64 --isa i,m,a,f,d,c,zicsr,zifencei \
  --ram-base 0x80000000 --ram-size 0x08000000 \
  --scratch-size 4096 --smc-size 4096 -o program.elf

cargo run -- one --xlen 32 --isa i,zicsr --smc-size 0 -o rv32.elf
```

The defaults reserve 128 MiB of physical address space starting at `0x80000000`
for allocation, with 4 KiB each for scratch and SMC. Only requested ELF sections
are initialized; an ELF does not contain all of that RAM.

For additional sections or multiple RAM banks, construct `MemoryPlan::new` with
`MemoryRegion` values and `SectionRequest` values. Retain the required `.tohost`,
`.fromhost`, and `.scratch` requests; `.smc` is optional. Each request specifies
its size, alignment, and permissions. Add a symbolic fixup in `code.rs` only if
startup needs to load the address into a register. Runtime behavior remains
separate from the allocator.

Add supported ISA capabilities in `Target`; add dependency rules in its
constructor when needed. Add startup initialization in `code.rs` guarded by
`Target::has`. There is no need to add parallel target fields to generation.

Run `cargo test --offline` for target, startup, allocation, bounds, and generation
regression tests.
