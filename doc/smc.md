# Self-modifying code (SMC): plan

Status: implemented (`--smc-proba`, default 0.1). Test:
`orchestrator::tests::smc_blocks_are_stored_at_run_time`.

SMC blocks are workload blocks whose code is not in the ELF. Earlier code
builds their bytes with instructions and stores them into place. A later
`fence.i` makes the stores visible to instruction fetch, then the block runs.
A core that ignores the `fence.i`, or whose I-cache is not kept coherent with
its data side, runs stale bytes instead and fails.

Code: `src/basicblock.rs` (`SmcStore`, `prepend`), `src/hart.rs` (`plan_smc`,
`place`, `link`, `image`), `src/orchestrator.rs` (call site, `.text`
permissions). Needs `zifencei` in `--isa`.

## Design

Generation stays the same: blocks are generated first, with `is_smc` already
drawn in `Hart::new`. A new step, `Hart::plan_smc`, runs **between
`Hart::run` and `Hart::place`**. It inserts code, which changes block sizes,
and `place` fixes addresses from those sizes. `link` only fills the inserted
placeholders in. For each SMC block `S`:

| Piece | Where | What |
|---|---|---|
| **Store site** | start of a random earlier block `H` | `auipc/addi rptr, S`; per chunk `lui/addi rval, bytes; sb/sh/sw rval, off(rptr)`; `addi rval, x0, 0` |
| **`fence.i`** | start of a random block `F`, `H <= F < S` | `fence.i` |
| **Old contents** | `S`'s address in the ELF | another block's code (alias mode) or random bytes (fresh mode) |

The order follows from rvgen's existing rule that blocks run once, in index
order (`probe_pcs` relies on it too):

```
A (alias only)  <  H (store)  <=  F (fence.i)  <  S
```

Two modes:

- **Alias**: `S` reuses the address of an earlier non-SMC block `A` that has
  already run. This tests the classic SMC bug: a stale I-cache line or fetch
  buffer for code that has already run.
- **Fresh**: `S` gets its own address and the ELF holds random fill there.
  This tests whether a cold I-cache miss sees the stores still held in the
  D-cache.

Alias mode is used half of the time when its conditions hold (step 5),
fresh mode otherwise.

### Rules and why

- **Insert only at block starts.** Every value, link and leave sequence lives
  inside one block, so no site gets split. `prepend` shifts every recorded
  index (`sites`, `fail_at`, `pool_refs`, `smc_stores`).
- **Hosts are never SMC blocks or landing blocks.** `S`'s bytes are then
  final once its own sites are rendered, so one render pass is enough. A
  landing block's first instruction is a trap target.
- **`H > A`.** A store at the start of `A` would overwrite `A` before it ran.
- **`rval` is reset to 0.** Otherwise it holds the last code chunk, which
  differs between the draft and the final program, and every later probe
  would see different registers. `rptr` holds an address, which is the same
  in both.
- **Store widths:** each chunk's width is drawn from `{1, 2, 4}` bytes, limited
  to `unit = target.instruction_alignment()`, at a naturally aligned offset.
  Stores are therefore aligned even when misaligned accesses trap, and
  partial writes get exercised.
- **`S` is at most 2 KiB**, the reach of the 12-bit store offset. A larger block
  is unmarked.
- **`.text` becomes RWX.** The reserved `smc` section is dropped: it sits at
  the top of RAM, out of `jal`/branch reach of the code area. PMP's last entry
  already gives S and U RWX on all of RAM. Workload stores never target
  `.text`, which is private.
- **Spike** runs the draft through the real store → `fence.i` → execute path,
  so probes inside `S` are correct. Spike's `until pc` stops the first time
  an address is reached, which for a probe inside `S` could be `A` running at
  that address. So `probe_pcs` first stops Spike at the start of `F`, which
  runs after `A` at an address nothing ran at before. Spike always
  behaves as if `fence.i` worked, so whether the fence actually works can only
  be tested on the device under test.

## Steps

### 1. Writable code area (`orchestrator.rs`)

- In `Orchestrator::run`, change both `Permissions::RX` of the `.text` section
  to `Permissions::RWX`.
- Delete `state.memory.reserve(..., "smc", ...)` in `Orchestrator::new`.

### 2. `BasicBlock` fields (`basicblock.rs`)

```rust
/// Store sites writing SMC blocks' code, in `instrs`.
pub smc_stores: Vec<SmcStore>,
/// Index of the earlier block whose address this SMC block reuses.
pub alias: Option<usize>,
```

Set both to empty/`None` in `BasicBlock::new`. `Default` covers the
epilogue blocks.

### 3. Store site (`basicblock.rs`)

```rust
/// Writes block `smc`'s code through `rptr`, one store of `widths[k]` bytes
/// at a time; `rval` ends at 0 so draft and final leave the same registers.
#[derive(Debug, Clone)]
pub struct SmcStore { pub at: usize, pub smc: usize, pub rptr: XReg, pub rval: XReg, pub widths: Vec<usize> }

impl SmcStore {
    pub fn len(&self) -> usize { 2 + 3 * self.widths.len() + 1 }

    /// `pc` is the address of `instrs[self.at]`.
    pub fn render(&self, instrs: &mut [Instruction], pc: u64, smc_addr: u64, code: &[u8], xlen: Xlen) {
        use Instruction::*;
        let (hi, lo) = split_imm32(smc_addr.wrapping_sub(pc) as u32);
        let mut out = vec![Auipc { rd: self.rptr, imm: hi }, Addi { rd: self.rptr, rs1: self.rptr, imm: lo }];
        let mut off = 0;
        for &w in &self.widths {
            let v = code[off..off + w].iter().rev().fold(0u32, |acc, b| acc << 8 | *b as u32);
            out.extend(load_imm32(self.rval, v as i32, xlen));
            let (rs1, rs2, imm) = (self.rptr, self.rval, off as i32);
            out.push(match w { 1 => Sb { rs1, rs2, imm }, 2 => Sh { rs1, rs2, imm }, _ => Sw { rs1, rs2, imm } });
            off += w;
        }
        out.push(Addi { rd: self.rval, rs1: XReg::ZERO, imm: 0 });
        instrs[self.at..self.at + out.len()].copy_from_slice(&out);
    }
}

/// Store widths covering `size` bytes at an address aligned to `unit`:
/// each naturally aligned, mixed at random.
pub fn store_widths(size: usize, unit: usize, rng: &mut (impl Rng + ?Sized)) -> Vec<usize> {
    let mut widths = Vec::new();
    let mut off = 0;
    while off < size {
        let ok: Vec<_> = [1, 2, 4].into_iter().filter(|&w| w <= unit && off % w == 0 && off + w <= size).collect();
        let w = ok[rng.random_range(0..ok.len())];
        widths.push(w);
        off += w;
    }
    widths
}
```

### 4. `BasicBlock::prepend`

```rust
pub fn prepend(&mut self, instrs: &[Instruction]) {
    let n = instrs.len();
    self.instrs.splice(0..0, instrs.iter().copied());
    for s in &mut self.sites { s.at += n; if let Some(f) = &mut s.fail_at { *f += n; } }
    for r in &mut self.pool_refs { r.at += n; }
    for s in &mut self.smc_stores { s.at += n; }
}
```

`privilege.rs` records indices only through `bb.sites`, so nothing else needs
shifting.

### 5. `Hart::plan_smc` (`hart.rs`)

```rust
/// Give each SMC block a store site and a later `fence.i` at the start of
/// earlier non-SMC blocks and, when it can, an earlier block's address to
/// overwrite. Before `place`: it grows the code.
pub fn plan_smc(&mut self, target: &Target, rng: &mut (impl Rng + ?Sized)) -> Result<()> {
    let end = self.label_index(CHECK_LABEL).or(self.label_index(EXIT_LABEL)).expect("an epilogue");
    let unit = target.instruction_alignment();
    let reserved: Vec<_> = self.state.mem_bases.bases.iter().map(|b| b.reg).collect();
    let short = |bb: &BasicBlock| bb.sites.iter().any(|s| s.reach().is_some_and(|r| r < 1 << 20));
    let mut aliased = vec![false; self.bbs.len()];
    for smc in 3..end {
        let size = code_size(&self.bbs[smc].instrs);
        if !self.bbs[smc].is_smc { continue; }
        if !target.has(Extension::Zifencei) || size == 0 || size > 2048 {
            self.bbs[smc].is_smc = false;
            continue;
        }
        let usable = |bb: &BasicBlock| !bb.is_smc && bb.landing.is_none();
        // Alias: S starts its own chain, nothing follows it, and neither its
        // predecessor's jump into it nor its own jump out needs it nearby.
        let s = &self.bbs[smc];
        let free = s.after.is_none() && s.landing.is_none() && s.align == 0 && !short(s)
            && !short(&self.bbs[smc - 1]) && self.bbs.iter().all(|bb| bb.after != Some(smc));
        let olds: Vec<_> = (2..smc - 1).filter(|&a| free && usable(&self.bbs[a]) && !aliased[a]).collect();
        let alias = (!olds.is_empty() && rng.random_bool(0.5)).then(|| olds[rng.random_range(0..olds.len())]);
        let first_host = alias.map_or(2, |a| a + 1);
        let hosts: Vec<_> = (first_host..smc).filter(|&i| usable(&self.bbs[i])).collect();
        if hosts.is_empty() { self.bbs[smc].is_smc = false; continue; }
        let host = hosts[rng.random_range(0..hosts.len())];
        let later: Vec<_> = hosts.iter().copied().filter(|&i| i >= host).collect();
        let fence = later[rng.random_range(0..later.len())];
        let mut regs: Vec<_> = (1..32).map(|i| XReg::new(i).unwrap()).filter(|r| !reserved.contains(r)).collect();
        let rptr = regs.swap_remove(rng.random_range(0..regs.len()));
        let rval = regs.swap_remove(rng.random_range(0..regs.len()));
        let store = SmcStore { at: 0, smc, rptr, rval, widths: store_widths(size, unit, rng) };
        // Fence first: in the same block, the store then precedes it.
        self.bbs[fence].prepend(&[Instruction::FenceI]);
        self.bbs[host].prepend(&vec![Instruction::nop(); store.len()]);
        self.bbs[host].smc_stores.insert(0, store);
        if let Some(a) = alias {
            aliased[a] = true;
            self.bbs[smc].alias = Some(a);
        }
    }
    // Pad each overwritten block, past its exit, so its SMC block fits. Its size is final now.
    for smc in 0..self.bbs.len() {
        if let Some(a) = self.bbs[smc].alias {
            let missing = code_size(&self.bbs[smc].instrs).saturating_sub(code_size(&self.bbs[a].instrs));
            let pad = if unit == 2 { Instruction::cnop() } else { Instruction::nop() };
            self.bbs[a].instrs.extend(std::iter::repeat_n(pad, missing / unit));
        }
    }
    Ok(())
}
```

- **Why alias mode has those conditions:** `place` checks reach only for the
  first block of each chain. `S`'s address is forced to `A`'s, so nothing
  around `S` may need `S` to be nearby.
- **Why the padding is safe:** it sits after `A`'s exit, so it never runs. If
  `A` is a caller, the block after it is placed after the padding, and `ret`
  still goes to that block's address.

### 6. Call it, and handle aliases in `place`

In `Orchestrator::run`, right after the `core.run(...)` loop and before the
`.golden` and code-area sizing, which sum block sizes:

```rust
for hart in self.harts.iter_mut() { hart.plan_smc(&self.target, &mut self.rng)?; }
```

In `Hart::place`, at the top of `for first in 0..self.bbs.len()`:

```rust
if let Some(a) = self.bbs[first].alias {
    self.bbs[first].addr = self.bbs[a].addr; // A < S, so A is placed already
    placed[first] = true;
    continue;
}
```

This runs before `S + 1` is placed, which reads `S.addr` to compute its reach.

### 7. Render the stores (`hart.rs`)

```rust
fn render_smc_stores(&mut self, target: &Target) -> Result<()> {
    for host in 0..self.bbs.len() {
        for k in 0..self.bbs[host].smc_stores.len() {
            let store = self.bbs[host].smc_stores[k].clone();
            let code = self.bbs[store.smc].encode(target)?;
            let smc_addr = self.bbs[store.smc].addr;
            let bb = &mut self.bbs[host];
            let pc = bb.addr + code_size(&bb.instrs[..store.at]) as u64;
            store.render(&mut bb.instrs, pc, smc_addr, &code, target.xlen);
        }
    }
    Ok(())
}
```

- Give `Hart::link` a `target: &Target` parameter (`Orchestrator::link`
  passes `&self.target`).
- Call this helper at the end of `link`, after the site loop so `S` is fully
  linked.
- Call it again at the end of `patch_entanglements`: `S`'s sites change after
  the Spike run, while the store site keeps its size.

### 8. Stale contents in the image (`Hart::image`)

```rust
for bb in blocks {
    if bb.alias.is_some() { continue; } // A's bytes are the old contents
    image.push(raw(at, bb.addr));
    image.push(if bb.is_smc { raw(bb.addr, bb.addr + code_size(&bb.instrs) as u64) } else { bb.clone() });
    at = bb.addr + code_size(&bb.instrs) as u64;
}
```

### 9. Settings and the check

- Set `is_smc` to `rng.random_bool(0.1)` in `Hart::new`. The store site costs
  about 3 instructions per chunk, so 50% blows up the code size.
- Add one test next to `scattered_programs_place_and_encode`: `isa = [I, M, C,
  Zicsr, Zifencei]`, self-check on, about 20 seeds. Assert that each SMC
  block's address range in the image differs from its encoding.
  `resolve_self_check` already fails if the program fails on Spike.

## Later

- **Entangled code words.** Give each store site one workload register `d` and one
  Spike probe at its first `lui`:
  - each chunk becomes `lui/addi rval, v ^ D; xor rval, rval, d`, where `D` is
    the low bits of `d` as reported by Spike;
  - in the draft, the `xor` reads x0, as value sites do;
  - add the probe to `probe_pcs` and resolve it in `patch_entanglements`, in
    the same index order.

  A miscomputed store value then breaks `S` itself.
- **`fence.i` inside a block.** It currently goes only at block starts.
  Mid-block placement would need to skip site ranges.
- **Overwriting an address more than once.** Each address is overwritten once.
  More versions would need blocks that run more than once.
