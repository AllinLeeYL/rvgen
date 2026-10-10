# Entanglement: value sites and guard sites

Entanglement (after the Cascade fuzzer) makes a program's path depend on the
values its workload computes. A core that miscomputes a value leaves the
planned path, usually straight into `_fail` (exit code `0xab`), instead of
carrying on until `_check` at the very end, by which time the bad value may
have been overwritten.

Code: `src/entangle.rs` (sites), `src/hart.rs` (link, probe, patch),
`src/orchestrator.rs` (`resolve_self_check`). Entanglement is on whenever the
self-check is (`--no-entangle` turns it off).

## The two building blocks

A **guard** is a pair of sites, emitted together by `SiteBuilder::guard`:

| Site | Instructions | Job |
|---|---|---|
| **Value site** | `r = golden; r ^= d1; ...; r ^= dk` | Fold the workload registers `d1..dk` into one register `r`, which ends up equal to a planned value `dest` on a correct core |
| **Guard site** | `beq/bne r, x0` around a `jal x0, _fail` | Check that `r` really is 0 (`dest = 0`) and fail otherwise |

`golden` is chosen as `dest ^ d1 ^ ... ^ dk`, so the XORs cancel:

```
r = golden ^ d1 ^ ... ^ dk = dest ^ (d1 ^ ... ^ dk) ^ (d1 ^ ... ^ dk) = dest
```

If any `di` is wrong, `r != dest`.

A value site is also used without a guard: with `dest` set to the next
block's address, it builds the target of an indirect `jalr`, so a wrong value
jumps somewhere random.

## The catch: golden needs values nobody knows yet

`golden` depends on the registers' values at that exact point of the program,
which depend on `_init`'s random values, loads from random memory, CSR reads,
traps... rvgen does not simulate the ISA itself; it asks Spike, in two passes:

```
generate (placeholders) → link → render draft → encode draft
  → Spike runs the draft, dumping registers at each site's probe PC
  → resolve + render final → encode final → Spike must exit 0
```

The draft must be runnable and take the same path as the final program, so
sites render a *placeholder* first:

| | Draft | Final |
|---|---|---|
| golden | `dest` | `dest ^ d1 ^ ... ^ dk` |
| XOR operands | `x0` | `d1 ... dk` |
| Value of `r` | `dest` (no workload data needed) | `dest` (only on a correct core) |

Every site has a **fixed size** in both forms, so patching moves no code:
addresses, jump offsets and probe PCs all stay valid.

## Example run

Real output of `rvgen one --num-instrs 6 --seed 3 --guard-threshold 0`,
disassembled with `riscv64-unknown-elf-objdump -d`:

```
# ── workload: each write is recorded in SiteBuilder.fresh ─────────
8007454c  slli  t5, a7, 0x32      # fresh = [t5]
80074550  addw  t6, t0, a7        # fresh = [t5, t6]
80074554  subw  a7, t4, a7        # fresh = [t5, t6, a7]
80074558  addi  a4, a4, -336      # fresh = [t5, t6, a7, a4]

# ── VALUE SITE: s7 = golden ^ t5 ^ t6 ^ a7 ^ a4 ─────────────────
8007455c  auipc s7, 0x7f78        # ┐ producer: load golden from
80074560  ld    s7, 1852(s7)      # ┘ .golden slot 0 (0x87fecc98)
80074564  xor   s7, s7, t5        # ┐ ← Spike probes registers here
80074568  xor   s7, s7, t6        # │ one XOR per fresh register
8007456c  xor   s7, s7, a7        # │ (fresh is now empty: consumed)
80074570  xor   s7, s7, a4        # ┘

# ── GUARD SITE: is s7 zero? ──────────────────────────────────
80074574  beqz  s7, 8007457c      # s7 == 0 → skip the fail jump
80074578  j     _fail             # s7 != 0 → exit code 0xab

# ── block end: jump to the next block, wherever it was placed ──
8007457c  j     80006ab8
80074580  ...random bytes...      # filler between blocks, never run
```

`.golden` slot 0 holds `0x88fab0ac9e25bc57`, which is `t5 ^ t6 ^ a7 ^ a4` as
Spike computed them.

### Step 1: generation (`SiteBuilder::guard`, `entangle.rs`)

The block reaches its end, so `block_end` appends a guard:

- The accumulator is a **stale** register, `s7`: one holding no unchecked
  workload value, so overwriting it loses nothing.
- All of `fresh = [t5, t6, a7, a4]` become the value site's `rdeps`, and
  `fresh` is emptied: guards *consume* fresh registers.
- The value site reserves 2 (producer) + 4 (XORs) `nop`s; the guard site
  reserves one for the branch and one for `j _fail`. A coin flip picked
  `taken = true` (`beqz` skipping the fail jump); `taken = false` would give
  `bnez` over a `j +8` instead.

### Step 2: link and draft (`Hart::link`, `Site::link`, `Site::render`)

Blocks now have addresses, so the `auipc; ld` gets its offset to slot 0, and
the draft is rendered:

```
ld   s7 ← .golden[0] = 0       # golden = dest = 0
xor  s7, s7, x0                # ×4: no-ops
beqz s7, +8                    # always taken
j    _fail
```

It passes whatever the registers hold.

### Step 3: Spike (`Hart::probe_pcs`, `Spike::states_at`)

Spike runs the draft and dumps all 32 registers when the PC reaches
`80074564`, the first XOR (`Site::probe_index`). The workload has run, so
`t5`, `t6`, `a7` and `a4` hold their real values.

### Step 4: patch (`Hart::patch_entanglements`, `Site::resolve`, `Site::render`)

- `rdep_val = t5 ^ t6 ^ a7 ^ a4 = 0x88fab0ac9e25bc57`
- golden `= dest ^ rdep_val = 0x88fab0ac9e25bc57`, written to `.golden[0]` by
  `Hart::golden_bytes`
- The XORs are rewritten to read `t5, t6, a7, a4` instead of `x0`.

Same instruction count, so nothing moves. The final image is run on Spike once
more and must exit 0.

### Step 5: on the device under test

| Core | `s7` after the XORs | `beqz` | Result |
|---|---|---|---|
| Correct | `0x88fa…57 ^ 0x88fa…57 = 0` | taken | next block |
| `subw` flips one bit of `a7` | that bit, `≠ 0` | not taken | `j _fail`, exit `0xab` |

## Where guards are placed

`SiteBuilder` records every register the workload writes in `fresh`
(`observe`). A guard consumes all of them and is placed:

- at the end of every block (`block_end`), and
- mid-block once `fresh.len() >= --guard-threshold` (default 8, 0 disables
  mid-block guards; `after_instr`), so fewer bad values get overwritten before
  a guard sees them. Lower catches more faults but makes longer programs.

A fault is only caught if the bad register reaches a guard before the workload
overwrites it.

## Where the golden value lives

- **RV64** (default): in the `.golden` data section, loaded with `auipc; ld`.
  `Site::render` leaves the producer alone; patching rewrites data, not code.
- **RV32, or `--inline-golden`**: materialized inline with a fixed-length
  `load_imm_fixed` sequence, which `Site::render` rewrites.

## Invariants to keep when changing this code

1. A site's draft and final forms have the **same instruction count**.
2. The draft takes the **same path** as the final program, so the register
   state at every site matches.
3. `Hart::probe_pcs` and `Hart::patch_entanglements` walk the sites in the
   **same order** (each block runs once, in execution order).
4. Site code never writes the memory base registers (`reserved`).

Breaking 1 or 2 shows up as the final image failing on Spike
("the self-checking program fails on Spike itself").
