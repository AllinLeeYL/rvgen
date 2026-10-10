use std::collections::{BTreeMap, HashMap};

use anyhow::{Ok, Result, anyhow, ensure};
use rand::rngs::StdRng;
use rand::{Rng, RngExt};

use crate::basicblock::{BasicBlock, SmcStore, store_widths};
use crate::entangle::{GOLDEN_SLOT_SIZE, PoolRef, SiteBuilder, code_size, use_compressed};
use crate::membase::MemBases;
use crate::orchestrator::PMP_WINDOW_SECTION;
use crate::privilege::{Ctx, MSTATUS_CLEARED, PMP_REGION, PmpWindow, PrivState, SINK_M, SINK_S, Switcher, window_perms};
use crate::orchestrator::GlobalState;
use crate::riscv::asmutil::{load_imm, load_imm32, load_imm_fixed, twos_complement};
use crate::riscv::{Csr, Extension, FReg, Instruction, Opcode, PrivilegeLevel, XReg, Xlen};
use crate::spike::ArchState;
use crate::target::Target;
use crate::utils::cut_cake_randomly;
use crate::weights::InstrWeights;

/// Labels, exported as ELF symbols, of the parts of a hart's code.
pub const INIT_LABEL: &str = "_init";
pub const STIMULUS_LABEL: &str = "_stimulus";
pub const CHECK_LABEL: &str = "_check";
pub const EXIT_LABEL: &str = "_exit";
pub const FAIL_LABEL: &str = "_fail";
pub const TRAP_HANDLER_LABEL: &str = "_trap_handler";
pub const STRAP_HANDLER_LABEL: &str = "_strap_handler";

/// `_strap_handler` reports `scause` plus this, above every `mcause + 1` the
/// M-mode handler reports for an exception.
pub const STRAP_EXIT_CODE_BASE: i32 = 0x101;

/// HTIF exit code reported when the self-check finds a mismatch. It lies above
/// every `mcause + 1` the trap handler reports for exceptions, and its low byte
/// is nonzero so it survives truncation to an 8-bit process exit status.
pub const MISMATCH_EXIT_CODE: i32 = 0xaa;

/// HTIF exit code reported by `_fail`, which an entangled branch or indirect
/// jump reaches when it goes the wrong way.
pub const DIVERGENCE_EXIT_CODE: i32 = 0xab;

/// Deepest nesting of calls.
const MAX_CALL_DEPTH: usize = 8;

#[derive(Debug, Clone, Default)]
pub struct HartState {
    /// The privilege mode and the state the switches set up, as of the block
    /// being generated (see [`crate::privilege`]).
    pub privilege: PrivState,
    /// Registers reserved to address memory, fixed for the whole program.
    pub mem_bases: MemBases,
}

/// `.golden` holds the sites' golden values, then, unless the constants are
/// inline, `_init`'s values and `_check`'s expected values.
#[derive(Default)]
pub struct Hart {
    // Constants & Targets
    /// Whether `_init` and `_check` load their constants from `.golden`.
    pooled: bool,
    /// Golden values the entanglement sites load from `.golden`.
    site_slots: usize,
    /// Values `_init` writes, see [`init_values`].
    init_values: Vec<u64>,
    /// Constants `_check` compares against, see [`check_values`].
    check_values: Vec<u64>,
    /// Start of the code area and its bytes between blocks, see [`Hart::place`].
    area: u64,
    fill: Vec<u8>,

    // Variables & Mutables
    pub bbs: Vec<BasicBlock>,
    state: HartState,
}

impl Hart {
    pub fn new(num_instrs: usize, smc_proba: f64, rng: &mut StdRng) -> Self {
        // Randomly distribute budgets to basic blocks
        let core = Self {
            state: HartState::default(),
            bbs: cut_cake_randomly(num_instrs, Some(1), Some(32), rng)
                .into_iter()
                .enumerate()
                .map(|(id, budget)| BasicBlock::new(id, rng.random_bool(smc_proba), budget))
                .collect(),
            ..Default::default()
        };
        debug_assert_eq!(
            core.bbs.iter().map(|bb| bb.budget).sum::<usize>(),
            num_instrs,
        );
        core
    }

    /// Generate the hart's code, drawing each workload block's opcodes from
    /// `weights`. Every block ends with a way to the next one in execution
    /// order: a taken branch or `jalr` site, a `jal`/`c.j`, a call (`jal ra`)
    /// or, inside a callee, a return (`ret`/`c.jr ra`) into the block placed
    /// right after the caller. With `self_check`, a self-check block holding
    /// placeholder values precedes the exit; patch the reference values in with
    /// [`Hart::set_final_expected_values`] before encoding the final program.
    /// With `entangle` (holding the mid-block guard threshold), the workload
    /// holds entanglement sites; [`Hart::place`] and [`Hart::link`] them, then
    /// [`Hart::patch_entanglements`] with Spike's register values. With
    /// `golden_section` (RV64 only), sites, `_init` and `_check` load their
    /// constants from a `.golden` section of [`Hart::golden_slots`] slots,
    /// which the caller reserves before linking.
    pub fn run(
        &mut self,
        rng: &mut (impl Rng + ?Sized),
        target: &Target,
        state: &mut GlobalState,
        weights: &InstrWeights,
        self_check: bool,
        entangle: Option<usize>,
        golden_section: bool,
    ) -> Result<()> {
        let sections: Vec<_> = state.memory.data_sections().collect();
        self.state.mem_bases = MemBases::pick(&sections, rng)?;
        let reserved: Vec<_> = self.state.mem_bases.bases.iter().map(|base| base.reg).collect();
        let mut sites =
            entangle.map(|threshold| SiteBuilder::new(target, threshold, reserved.clone(), golden_section));
        self.pooled = golden_section;
        let tohost = state.memory.get(".tohost")?.region.start;

        // Code parts in execution order and the symbol marking each:
        //   _start         point mtvec at the handler (entry point)
        //   _init          randomize registers, the memory bases getting their addresses
        //   _stimulus      workload, its blocks scattered over the code area
        //   _check         (optional) compare all registers against Spike's values
        //   _exit          report the check's verdict (or success) to tohost and spin
        //   _fail          (with entanglement) report a divergence to tohost and spin
        //                  (padding to 4 bytes, never executed)
        //   _trap_handler  report mcause to tohost and spin
        // All but the workload are placed back to back, in this order, at the
        // start of the code area. mtvec is set first so a trap anywhere after
        // it terminates the test. `_start` is exported by the ELF encoder for
        // the whole code, so the mtvec block carries no label of its own. The
        // mtvec and `_init` blocks are filled in once the workload is known.
        if self.bbs.is_empty() {
            self.bbs.push(BasicBlock::new(0, false, 0));
        }
        self.bbs[0].label = Some(STIMULUS_LABEL.into());
        self.bbs.splice(0..0, [BasicBlock::default(), BasicBlock::default()]);

        // A call makes the next block a callee, which returns into the block
        // placed right after the caller. Calls nest up to MAX_CALL_DEPTH;
        // the innermost callee returns first.
        // Privilege switches end blocks outside calls: a trap's next block is
        // its landing, which starts by restoring the trap vector, and an
        // `xret`'s next block runs in the new mode. The program is back in M
        // when it reaches `_check`.
        let window = state.memory.get(PMP_WINDOW_SECTION).ok().map(|section| PmpWindow {
            start: section.region.start,
            perms: window_perms((section.region.size / PMP_REGION) as usize, rng),
        });
        let mut switcher = Switcher::new(target, window, rng);
        let initial_medeleg = switcher.state.medeleg;
        let mem_bases = self.state.mem_bases.clone();
        let mut landing = None;

        let call_proba = rng.random_range(0.0..0.4);
        let mut callers = Vec::new();
        let mut index = 2;
        while index < self.bbs.len() {
            let last = index + 1 == self.bbs.len();
            let bb = &mut self.bbs[index];
            if let Some(landing) = landing.take() {
                let mut ctx = Ctx { target, sites: sites.as_mut(), reserved: &reserved, mem_bases: &mem_bases };
                switcher.land(bb, &mut ctx, landing, rng);
            }
            self.state.privilege = switcher.state.clone();
            if let Some(left) = bb.run(rng, target, state, &mut self.state, weights, sites.as_mut())? {
                // A control-flow site ended the block. The rest of its budget
                // starts the next one; the last block also needs a plain jump,
                // the only way out sure to reach `_check`.
                if left > 0 || last {
                    self.bbs.insert(index + 1, BasicBlock::new(index + 1, false, left));
                }
            } else if !callers.is_empty() && (last || rng.random_bool(0.5)) {
                bb.ret(use_compressed(target, &[Opcode::CJr], rng));
                if last {
                    self.bbs.push(BasicBlock::new(index + 1, false, 0));
                }
                self.bbs[index + 1].after = callers.pop();
            } else if callers.is_empty() && {
                let mut ctx = Ctx { target, sites: sites.as_mut(), reserved: &reserved, mem_bases: &mem_bases };
                let (ended, next) = switcher.end_block(bb, &mut ctx, last, rng)?;
                landing = next;
                ended
            } {
                if last {
                    self.bbs.push(BasicBlock::new(index + 1, false, 0));
                }
            } else {
                let call = !last && callers.len() < MAX_CALL_DEPTH && rng.random_bool(call_proba);
                let compressed = !call && !last && use_compressed(target, &[Opcode::CJ], rng);
                bb.jump(if call { XReg::RA } else { XReg::ZERO }, compressed);
                if call {
                    callers.push(index);
                }
            }
            index += 1;
        }
        self.site_slots = sites.as_ref().map_or(0, SiteBuilder::golden_slots);

        let init_csrs = crate::csrs::init_csrs(target, initial_medeleg, switcher.window(), rng);
        self.init_values = init_values(rng, target, &init_csrs, &self.state.mem_bases);
        let csrs: Vec<_> = init_csrs.iter().map(|(csr, _)| *csr).collect();
        let mut init = init_block(&self.init_values, &csrs, target, self.init_slot())?;
        init.label = Some(INIT_LABEL.into());
        init.after = Some(0);
        init.jump(XReg::ZERO, false);
        self.bbs[1] = init;

        let mut epilogue = Vec::new();
        if self_check {
            self.check_values = check_values(&ArchState::default(), target);
            epilogue.push(check_block(&self.check_values, target, self.check_slot())?);
        }
        let verdict = if self_check {
            // The check leaves 0 or MISMATCH_EXIT_CODE in x5.
            Instruction::Slli {
                rd: XReg::X5,
                rs1: XReg::X5,
                shamt: 1,
            }
        } else {
            Instruction::Addi {
                rd: XReg::X5,
                rs1: XReg::ZERO,
                imm: 1,
            }
        };
        epilogue.push(BasicBlock {
            instrs: report_to_tohost(verdict, tohost, target.xlen),
            label: Some(EXIT_LABEL.into()),
            ..Default::default()
        });
        {
            // Also reached by falling through a trap or an xret.
            let fail = Instruction::Addi {
                rd: XReg::X5,
                rs1: XReg::ZERO,
                imm: DIVERGENCE_EXIT_CODE << 1,
            };
            epilogue.push(BasicBlock {
                instrs: report_to_tohost(fail, tohost, target.xlen),
                label: Some(FAIL_LABEL.into()),
                ..Default::default()
            });
        }
        let has_s = target.has_privilege(PrivilegeLevel::Supervisor);
        let start_len = code_size(&set_tvec(Csr::MTVEC, 0)) * if has_s { 2 } else { 1 };
        let before_handler = start_len
            + code_size(&self.bbs[1].instrs)
            + epilogue.iter().map(|bb| code_size(&bb.instrs)).sum::<usize>();
        let mut padding = Vec::new();
        if before_handler % 4 != 0 {
            // Only reachable with C enabled; never executed.
            padding.push(Instruction::cnop());
        }
        let handler_offset = before_handler + code_size(&padding);
        epilogue.push(BasicBlock {
            instrs: padding,
            ..Default::default()
        });
        let handler = trap_handler(tohost, target.xlen);
        // Every handler instruction is 4 bytes, so the next handler is aligned too.
        let strap_offset = handler_offset + code_size(&handler);
        epilogue.push(BasicBlock {
            instrs: handler,
            label: Some(TRAP_HANDLER_LABEL.into()),
            landing: Some(SINK_M),
            ..Default::default()
        });
        self.bbs[0].instrs = set_tvec(Csr::MTVEC, handler_offset as i64).to_vec();
        if has_s {
            epilogue.push(BasicBlock {
                instrs: strap_handler(tohost, target.xlen),
                label: Some(STRAP_HANDLER_LABEL.into()),
                landing: Some(SINK_S),
                ..Default::default()
            });
            let at = code_size(&self.bbs[0].instrs);
            self.bbs[0].instrs.extend(set_tvec(Csr::STVEC, (strap_offset - at) as i64));
        }
        // The epilogue follows `_init`, each block right after the previous one.
        let mut prev = 1;
        for mut bb in epilogue {
            bb.after = Some(prev);
            prev = self.bbs.len();
            self.bbs.push(bb);
        }
        Ok(())
    }

    /// Give each SMC block a store site writing its code and a later
    /// `fence.i`, each at the start of a random earlier non-SMC block, and,
    /// when it can, the address of an earlier block that has run by then, so
    /// the stores overwrite executed code. Blocks run once, in index order:
    /// old block < store < fence.i < SMC block. Before [`Hart::place`], as it
    /// grows the code. See `doc/smc.md`.
    pub fn plan_smc(&mut self, target: &Target, rng: &mut (impl Rng + ?Sized)) -> Result<()> {
        let end = self
            .label_index(CHECK_LABEL)
            .or_else(|| self.label_index(EXIT_LABEL))
            .ok_or_else(|| anyhow!("hart has no epilogue"))?;
        let unit = target.instruction_alignment();
        let reserved: Vec<_> = self.state.mem_bases.bases.iter().map(|base| base.reg).collect();
        let short = |bb: &BasicBlock| bb.sites.iter().any(|site| site.reach().is_some_and(|reach| reach < 1 << 20));
        let usable = |bb: &BasicBlock| !bb.is_smc && bb.landing.is_none();
        let mut aliased = vec![false; self.bbs.len()];
        for smc in 0..self.bbs.len() {
            if !self.bbs[smc].is_smc {
                continue;
            }
            let size = code_size(&self.bbs[smc].instrs);
            // Stores reach 2 KiB past their pointer.
            if smc < 3 || smc >= end || !target.has(Extension::Zifencei) || size == 0 || size > 2048 {
                self.bbs[smc].is_smc = false;
                continue;
            }
            // Its address is forced to the old block's, so nothing may need
            // it nearby: it starts its own chain, nothing follows it, and
            // neither the jump into it nor the one out of it is short.
            let s = &self.bbs[smc];
            let free = s.after.is_none()
                && s.landing.is_none()
                && s.align == 0
                && !short(s)
                && !short(&self.bbs[smc - 1])
                && self.bbs.iter().all(|bb| bb.after != Some(smc));
            let olds: Vec<_> = (2..smc - 1)
                // Padding the old block must not stretch a short jump out of it.
                .filter(|&old| free && usable(&self.bbs[old]) && !short(&self.bbs[old]) && !aliased[old])
                .collect();
            let alias = (!olds.is_empty() && rng.random_bool(0.5)).then(|| olds[rng.random_range(0..olds.len())]);
            // A store at the start of the old block would overwrite it before it runs.
            let first_host = alias.map_or(2, |old| old + 1);
            let hosts: Vec<_> = (first_host..smc).filter(|&index| usable(&self.bbs[index])).collect();
            if hosts.is_empty() {
                self.bbs[smc].is_smc = false;
                continue;
            }
            let host = hosts[rng.random_range(0..hosts.len())];
            let later: Vec<_> = hosts.iter().copied().filter(|&index| index >= host).collect();
            let fence = later[rng.random_range(0..later.len())];
            let mut regs: Vec<_> = (1..32)
                .map(|index| XReg::new(index).expect("valid register"))
                .filter(|reg| !reserved.contains(reg))
                .collect();
            let rptr = regs.swap_remove(rng.random_range(0..regs.len()));
            let rval = regs.swap_remove(rng.random_range(0..regs.len()));
            let store = SmcStore { at: 0, smc, rptr, rval, widths: store_widths(size, unit, rng) };
            // The fence goes in first: in the same block, the store precedes it.
            self.bbs[fence].prepend(&[Instruction::FenceI]);
            self.bbs[host].prepend(&vec![Instruction::nop(); store.len()]);
            self.bbs[host].smc_stores.insert(0, store);
            if let Some(old) = alias {
                aliased[old] = true;
                self.bbs[smc].alias = Some(old);
            }
        }
        // Pad each old block past its exit, never executed, so its SMC block
        // fits. Sizes are final now.
        let pad = if unit == 2 { Instruction::cnop() } else { Instruction::nop() };
        for smc in 0..self.bbs.len() {
            if let Some(old) = self.bbs[smc].alias {
                let missing = code_size(&self.bbs[smc].instrs).saturating_sub(code_size(&self.bbs[old].instrs));
                self.bbs[old].instrs.extend(std::iter::repeat_n(pad, missing / unit));
            }
        }
        Ok(())
    }

    /// Write each SMC block's current code into the store sites writing it.
    /// Store sites are never in SMC blocks, so one pass will do.
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

    /// Address of the self-check block.
    pub fn self_check_pc(&self) -> Result<u64> {
        Ok(self.bbs[self.get_self_check_bb_index()?].addr)
    }

    /// Number of constants loaded from `.golden`.
    pub fn golden_slots(&self) -> usize {
        self.site_slots + if self.pooled { self.init_values.len() + self.check_values.len() } else { 0 }
    }

    /// First `.golden` slot of `_init`'s values, if pooled.
    fn init_slot(&self) -> Option<usize> {
        self.pooled.then_some(self.site_slots)
    }

    /// First `.golden` slot of `_check`'s expected values, if pooled.
    fn check_slot(&self) -> Option<usize> {
        self.pooled.then_some(self.site_slots + self.init_values.len())
    }

    /// Contents of the `.golden` section, little endian: each linked site's
    /// golden value in its slot (zero before linking), then `_init`'s and
    /// `_check`'s constants if pooled. The draft holds the sites' and
    /// `_check`'s placeholders.
    pub fn golden_bytes(&self) -> Vec<u8> {
        let mut slots = vec![0; self.golden_slots()];
        for site in self.bbs.iter().flat_map(|bb| &bb.sites) {
            if let Some((slot, golden)) = site.golden_entry() {
                slots[slot] = golden;
            }
        }
        if self.pooled {
            let constants = self.init_values.iter().chain(&self.check_values);
            slots[self.site_slots..].iter_mut().zip(constants).for_each(|(slot, value)| *slot = *value);
        }
        slots.iter().flat_map(|slot| slot.to_le_bytes()).collect()
    }

    /// Place the blocks at random in the code area of `size` bytes at
    /// `start`. Blocks chained by [`BasicBlock::after`] go back to back, each
    /// chain where the jump into it reaches; the first one, all code but the
    /// workload, at `start`. The gaps get random bytes, which wrong-path
    /// fetches decode.
    pub fn place(&mut self, start: u64, size: u64, align: u64, rng: &mut (impl Rng + ?Sized)) -> Result<()> {
        let end = start + size;
        let sizes: Vec<u64> = self.bbs.iter().map(|bb| code_size(&bb.instrs) as u64).collect();
        let mut follower = vec![None; self.bbs.len()];
        for (index, bb) in self.bbs.iter().enumerate() {
            if let Some(after) = bb.after {
                follower[after] = Some(index);
            }
        }
        let chains: Vec<Vec<usize>> = (0..self.bbs.len())
            .map(|head| std::iter::successors(Some(head), |&index| follower[index]).collect())
            .collect();
        // Whether a block jumps to the next one with less than the whole
        // area's reach. Only a chain's last block can: the others are callers
        // (`jal ra`) or fixed code.
        let short: Vec<bool> = self
            .bbs
            .iter()
            .map(|bb| bb.sites.iter().any(|site| site.reach().is_some_and(|reach| reach < 1 << 20)))
            .collect();
        // Bytes of a head's chain and, back to back after it, of every chain
        // it reaches with short jumps. Placing them right after one another
        // always works, so a head only goes where its `total` fits.
        let mut total = vec![0; self.bbs.len()];
        for head in (0..self.bbs.len()).rev() {
            let last = *chains[head].last().expect("a chain holds its head");
            total[head] = chains[head].iter().map(|&index| sizes[index]).sum::<u64>()
                + if short[last] { total[last + 1] } else { 0 };
        }
        // Occupied ranges, start -> end.
        let mut used = BTreeMap::new();
        let mut placed = vec![false; self.bbs.len()];
        for first in 0..self.bbs.len() {
            // An SMC block overwriting an old one takes its address; the old
            // one comes first, so it is placed already.
            if let Some(old) = self.bbs[first].alias {
                self.bbs[first].addr = self.bbs[old].addr;
                placed[first] = true;
                continue;
            }
            let mut head = first;
            // Place `head`'s chain, then right away the chain its last block
            // reaches with a short jump, and so on, while the room is free.
            while self.bbs[head].after.is_none() && !placed[head] {
                // The previous block in execution order is placed already.
                let (lo, hi) = match head.checked_sub(1).map(|prev| &self.bbs[prev]) {
                    None => (start, start),
                    Some(prev) => match prev.sites.iter().find_map(|site| Some((site.at, site.reach()?))) {
                        Some((at, reach)) => {
                            let pc = prev.addr + code_size(&prev.instrs[..at]) as u64;
                            (pc.saturating_sub(reach), pc + reach - 2)
                        }
                        None => (start, end),
                    },
                };
                let len = total[head];
                // Trap vectors need 4-byte aligned landings, vectored ones more.
                let bb = &self.bbs[head];
                let align = if bb.landing.is_some() { align.max(4) } else { align }.max(bb.align);
                let addr = free_spot(&used, start, end, lo, hi, len, align, rng)
                    .ok_or_else(|| anyhow!("no room for {len} bytes of code within reach; the code area is too full"))?;
                let mut at = addr;
                for &index in &chains[head] {
                    self.bbs[index].addr = at;
                    placed[index] = true;
                    at += sizes[index];
                }
                used.insert(addr, at);
                let last = *chains[head].last().expect("a chain holds its head");
                if !short[last] {
                    break;
                }
                head = last + 1;
            }
        }
        self.area = start;
        self.fill = vec![0; size as usize];
        rng.fill(&mut self.fill[..]);
        Ok(())
    }

    /// The code area in address order: the blocks, and the random fill as
    /// raw halfwords between them.
    pub fn image(&self) -> Vec<BasicBlock> {
        let raw = |from: u64, to: u64| BasicBlock {
            instrs: self.fill[(from - self.area) as usize..(to - self.area) as usize]
                .chunks(2)
                .map(|pair| Instruction::compressed(u16::from_le_bytes([pair[0], pair[1]])))
                .collect(),
            ..Default::default()
        };
        let mut blocks: Vec<_> = self.bbs.iter().collect();
        blocks.sort_by_key(|bb| (bb.addr, code_size(&bb.instrs)));
        let mut image = Vec::new();
        let mut at = self.area;
        for bb in blocks {
            // An SMC block's bytes are stored at run time: the old block's
            // code, or random fill, is there in the image.
            if bb.alias.is_some() {
                continue;
            }
            image.push(raw(at, bb.addr));
            let end = bb.addr + code_size(&bb.instrs) as u64;
            image.push(if bb.is_smc { raw(bb.addr, end) } else { bb.clone() });
            at = end;
        }
        image.push(raw(at, self.area + self.fill.len() as u64));
        image
    }

    /// Fix the code's addresses, given the blocks' places and that `.golden`
    /// starts at `golden_addr`, and render the sites' placeholders.
    pub fn link(&mut self, golden_addr: Option<u64>, target: &Target) -> Result<()> {
        let xlen = target.xlen;
        let fail_addr = self.label_index(FAIL_LABEL).map(|fail| self.bbs[fail].addr);
        let landings: HashMap<_, _> =
            self.bbs.iter().filter_map(|bb| Some((bb.landing?, bb.addr))).collect();
        let next: Vec<_> = self.bbs.iter().skip(1).map(|bb| Some(bb.addr)).chain([None]).collect();
        for (bb, next_addr) in self.bbs.iter_mut().zip(next) {
            for pool_ref in &bb.pool_refs {
                pool_ref.link(&mut bb.instrs, bb.addr, golden_addr)?;
            }
            for site in bb.sites.iter_mut() {
                site.link(&mut bb.instrs, bb.addr, next_addr, fail_addr, golden_addr, &landings)?;
            }
            bb.render_sites(xlen)?;
        }
        self.render_smc_stores(target)
    }

    /// Addresses at which Spike must report the registers of each
    /// entanglement site, in the order they are reached: each block runs
    /// once, in execution order.
    pub fn probe_pcs(&self, xlen: Xlen) -> Vec<u64> {
        let mut pcs = Vec::new();
        for bb in &self.bbs {
            for site in &bb.sites {
                if let Some(index) = site.probe_index(xlen) {
                    pcs.push(bb.addr + code_size(&bb.instrs[..index]) as u64);
                }
            }
        }
        pcs
    }

    /// Patch Spike's register values, one per site in [`Hart::probe_pcs`]
    /// order, into the entanglement sites. Sites keep their sizes.
    pub fn patch_entanglements(
        &mut self,
        states: &[ArchState],
        target: &Target,
        rng: &mut (impl Rng + ?Sized),
    ) -> Result<()> {
        let mut states = states.iter();
        for bb in self.bbs.iter_mut() {
            for site in bb.sites.iter_mut() {
                if site.probe_index(target.xlen).is_none() {
                    continue;
                }
                let state = states
                    .next()
                    .ok_or_else(|| anyhow!("missing register values for a site"))?;
                site.resolve(&state.xregs, target, rng)?;
            }
            bb.render_sites(target.xlen)?;
        }
        ensure!(states.next().is_none(), "more register values than sites");
        // SMC blocks' sites changed; their store sites keep their sizes.
        self.render_smc_stores(target)
    }

    fn label_index(&self, label: &str) -> Option<usize> {
        self.bbs.iter().position(|bb| bb.label.as_deref() == Some(label))
    }

    /// In the end of the program, there is a segment of self-checking code. 
    /// This func set the expected values: in `.golden` if pooled, in the code
    /// otherwise.
    pub fn set_final_expected_values(&mut self, expected: &ArchState, target: &Target) -> Result<()> {
        let index = self.get_self_check_bb_index()?;
        self.check_values = check_values(expected, target);
        if !self.pooled {
            let check = check_block(&self.check_values, target, None)?;
            ensure!(
                code_size(&check.instrs) == code_size(&self.bbs[index].instrs),
                "self-check block changed size while patching"
            );
            self.bbs[index].instrs = check.instrs;
        }
        Ok(())
    }

    fn get_self_check_bb_index(&self) -> Result<usize> {
        self.label_index(CHECK_LABEL)
            .ok_or_else(|| anyhow!("hart has no self-check block"))
    }
}

/// Initialize values in `_init`. The memory bases hold their addresses, the registers random values.
/// Layout: 
/// 1. [`crate::csrs::init_csrs`],
/// 2. f0-f32
/// 3. x1-x31
fn init_values(
    rng: &mut (impl Rng + ?Sized),
    target: &Target,
    csrs: &[(Csr, u64)],
    bases: &MemBases,
) -> Vec<u64> {
    let fregs = if target.has(Extension::F) { 32 } else { 0 };
    let mut values: Vec<_> = csrs.iter().map(|(_, value)| *value).collect();
    values.extend((0..fregs + 31).map(|_| random_xlen(rng, target.xlen) as u64));
    let x1 = values.len() - 31;
    for base in &bases.bases {
        values[x1 + base.reg.index() as usize - 1] = base.addr;
    }
    values
}

/// `_init`: enable the FPU (with F), then write [`init_values`] to `csrs`,
/// the FP registers and x1-x31 in that order, the CSRs and FP registers
/// through x5, so no scratch value leaks into the workload. With `pool`, the
/// values are loaded from `.golden` slots from `pool` on through x31, which is
/// loaded last. x0 is hardwired to zero.
fn init_block(values: &[u64], csrs: &[Csr], target: &Target, pool: Option<usize>) -> Result<BasicBlock> {
    use Instruction::*;
    const PTR: XReg = XReg::X31;
    let xlen = target.xlen;
    let mut block = BasicBlock::default();
    if let Some(slot) = pool {
        let pointer = PoolRef { at: 0, rd: PTR, slot, load: false };
        block.instrs.extend(pointer.instrs(0));
        block.pool_refs.push(pointer);
    }
    let offset = |k: usize| (k as u64 * GOLDEN_SLOT_SIZE) as i32;
    // rd = values[k]
    let load = |rd: XReg, k: usize| match pool {
        Some(_) => vec![Ld { rd, rs1: PTR, imm: offset(k) }],
        None => load_imm(rd, values[k] as i64, xlen),
    };
    let mut ks = 0..values.len();
    // No interrupt may fire, even below M where M-mode ones are always
    // enabled; mstatus starts with no virtualization and previous modes U.
    block.instrs.push(Csrrw { rd: XReg::ZERO, rs1: XReg::ZERO, csr: Csr::MIE });
    block.instrs.extend(load_imm32(XReg::X5, MSTATUS_CLEARED as i32, xlen));
    block.instrs.push(Csrrc { rd: XReg::ZERO, rs1: XReg::X5, csr: Csr::MSTATUS });
    if target.has(Extension::F) {
        // Enable the FPU first, as fcsr is among the CSRs: set both mstatus.FS
        // bits (Dirty) with CSRRS so the other mstatus fields are preserved.
        block.instrs.push(Lui {
            rd: XReg::X5,
            imm: 0x6, // 0x6000 = 0b11 << 13 (mstatus.FS)
        });
        block.instrs.push(Csrrs { rd: XReg::ZERO, rs1: XReg::X5, csr: Csr::MSTATUS });
    }
    // Each value is legal for its CSR (see crate::csrs::init_csrs).
    for &csr in csrs {
        block.instrs.extend(load(XReg::X5, ks.next().expect("a CSR value")));
        block.instrs.push(Csrrw { rd: XReg::ZERO, rs1: XReg::X5, csr });
    }
    if target.has(Extension::F) {
        // A 64-bit pattern with D (FLD or FMV.D.X, which only exists on RV64D);
        // otherwise a random (NaN-boxed) single from the low 32 bits.
        let wide = target.has(Extension::D);
        for index in 0..32 {
            let rd = FReg::new(index)?;
            let k = ks.next().expect("an FP value");
            if pool.is_some() {
                let imm = offset(k);
                block.instrs.push(if wide { Fld { rd, rs1: PTR, imm } } else { Flw { rd, rs1: PTR, imm } });
            } else {
                block.instrs.extend(load(XReg::X5, k));
                block.instrs.push(if wide && xlen == Xlen::X64 {
                    FmvDX { rd, rs1: XReg::X5 }
                } else {
                    FmvWX { rd, rs1: XReg::X5 }
                });
            }
        }
    }
    for (index, k) in (1..32).zip(ks) {
        block.instrs.extend(load(XReg::new(index)?, k));
    }
    Ok(block)
}

/// Constants `_check` compares x1-x31, then f0-f31 (with F), against.
/// FMV.X.D moves all 64 bits but only exists on RV64D; otherwise FMV.X.W
/// moves the low 32 bits, sign-extended to XLEN.
fn check_values(expected: &ArchState, target: &Target) -> Vec<u64> {
    let xlen = target.xlen;
    let mut values: Vec<_> = expected.xregs[1..]
        .iter()
        .map(|value| twos_complement(*value, xlen) as u64)
        .collect();
    if target.has(Extension::F) {
        let wide = xlen == Xlen::X64 && target.has(Extension::D);
        values.extend(expected.fregs.iter().map(|raw| {
            if wide { *raw } else { *raw as u32 as i32 as i64 as u64 }
        }));
    }
    values
}

/// `_check`: compare every register with [`check_values`] and leave the
/// verdict in x5: 0 on a match, [`MISMATCH_EXIT_CODE`] otherwise. The
/// workload may use every register, so none is reserved: x31 is stashed in
/// mscratch to serve as the first scratch register, x1 becomes the accumulator
/// once checked, and each register checked is free to reuse afterwards. Inline
/// constants are loaded with [`load_imm_fixed`] so the block's size does not
/// depend on them. With `pool`, they are loaded from `.golden` slots from
/// `pool` on, through x2 once it is checked.
fn check_block(values: &[u64], target: &Target, pool: Option<usize>) -> Result<BasicBlock> {
    use Instruction::*;
    const ACC: XReg = XReg::X1;
    const TMP: XReg = XReg::X31;
    const PTR: XReg = XReg::X2;
    let xlen = target.xlen;

    // rd = values[k]
    let load = |block: &mut BasicBlock, rd: XReg, k: usize| match pool {
        None => block.instrs.extend(load_imm_fixed(rd, values[k] as i64, xlen)),
        // x1's and x2's are loaded before x2 is free to point at the pool.
        Some(slot) if k < 2 => {
            let direct = PoolRef { at: block.instrs.len(), rd, slot: slot + k, load: true };
            block.instrs.extend(direct.instrs(0));
            block.pool_refs.push(direct);
        }
        Some(_) => block.instrs.push(Ld {
            rd,
            rs1: PTR,
            imm: (k as u64 * GOLDEN_SLOT_SIZE) as i32,
        }),
    };
    // acc |= value ^ values[k], with `scratch` holding the constant.
    let fold = |block: &mut BasicBlock, value: XReg, scratch: XReg, k: usize| {
        load(block, scratch, k);
        block.instrs.push(Xor { rd: scratch, rs1: scratch, rs2: value });
        block.instrs.push(Or { rd: ACC, rs1: ACC, rs2: scratch });
    };

    let mut block = BasicBlock {
        label: Some(CHECK_LABEL.into()),
        ..Default::default()
    };
    block.instrs.push(Csrrw { rd: XReg::ZERO, rs1: TMP, csr: Csr::MSCRATCH });
    load(&mut block, TMP, 0);
    block.instrs.push(Xor { rd: ACC, rs1: ACC, rs2: TMP });
    fold(&mut block, XReg::X2, TMP, 1);
    if let Some(slot) = pool {
        let pointer = PoolRef { at: block.instrs.len(), rd: PTR, slot, load: false };
        block.instrs.extend(pointer.instrs(0));
        block.pool_refs.push(pointer);
    }
    for index in 3..31 {
        fold(&mut block, XReg::new(index)?, TMP, index as usize - 1);
    }
    // Restore x31 and check it with x3, which is free by now.
    block.instrs.push(Csrrw { rd: TMP, rs1: XReg::ZERO, csr: Csr::MSCRATCH });
    fold(&mut block, TMP, XReg::X3, 30);

    if target.has(Extension::F) {
        let wide = xlen == Xlen::X64 && target.has(Extension::D);
        for index in 0..32 {
            let rs1 = FReg::new(index)?;
            block.instrs.push(if wide { FmvXD { rd: TMP, rs1 } } else { FmvXW { rd: TMP, rs1 } });
            fold(&mut block, TMP, XReg::X3, 31 + index as usize);
        }
    }

    // x5 = acc != 0 ? MISMATCH_EXIT_CODE : 0, without a branch.
    block.instrs.push(Sltu { rd: XReg::X5, rs1: XReg::ZERO, rs2: ACC });
    block.instrs.push(Sub { rd: XReg::X5, rs1: XReg::ZERO, rs2: XReg::X5 });
    block.instrs.push(Andi { rd: XReg::X5, rs1: XReg::X5, imm: MISMATCH_EXIT_CODE });
    Ok(block)
}

/// Point the trap vector `csr` (direct mode) at `offset` bytes from the first
/// instruction of this sequence. Always three instructions so the offset can
/// be patched in once the code in between is known.
fn set_tvec(csr: Csr, offset: i64) -> Vec<Instruction> {
    let hi = (offset + 0x800) >> 12;
    let mut instrs: Vec<Instruction> = vec![];
    instrs.push(Instruction::Auipc {
        rd: XReg::X5,
        imm: hi as i32,
    });
    instrs.push(
        Instruction::Addi {
            rd: XReg::X5,
            rs1: XReg::X5,
            imm: (offset - (hi << 12)) as i32,
        }
    );
    instrs.push(
        Instruction::Csrrw {
            rd: XReg::ZERO,
            rs1: XReg::X5,
            csr,
        }
    );
    instrs
}

/// Machine-mode trap handler. Every trap reaching it (illegal instruction,
/// access fault, misaligned access, or anything else unexpected) ends the
/// test with a failing HTIF exit code of `mcause + 1`, so 0 still means pass.
fn trap_handler(tohost: u64, xlen: Xlen) -> Vec<Instruction> {
    let mut instrs = vec![
        Instruction::Csrrs {
            rd: XReg::X5,
            rs1: XReg::ZERO,
            csr: Csr::MCAUSE,
        },
        Instruction::Addi {
            rd: XReg::X5,
            rs1: XReg::X5,
            imm: 1,
        },
    ];
    instrs.extend(report_to_tohost(
        Instruction::Slli {
            rd: XReg::X5,
            rs1: XReg::X5,
            shamt: 1,
        },
        tohost,
        xlen,
    ));
    instrs
}

/// Supervisor-mode sink handler, which `stvec` points at unless a planned trap
/// armed it: ends the test with exit code `0x101 + scause`, so unplanned
/// traps delegated to S fail too.
fn strap_handler(tohost: u64, xlen: Xlen) -> Vec<Instruction> {
    let mut instrs = vec![
        Instruction::Csrrs {
            rd: XReg::X5,
            rs1: XReg::ZERO,
            csr: Csr::SCAUSE,
        },
        Instruction::Addi {
            rd: XReg::X5,
            rs1: XReg::X5,
            imm: STRAP_EXIT_CODE_BASE,
        },
    ];
    instrs.extend(report_to_tohost(
        Instruction::Slli {
            rd: XReg::X5,
            rs1: XReg::X5,
            shamt: 1,
        },
        tohost,
        xlen,
    ));
    instrs
}

/// Write an HTIF exit command to `tohost` and spin until the host stops the
/// simulation. `encode` leaves `exit_code << 1` or the complete command in x5;
/// the low bit is then set, which HTIF reads as `(exit_code << 1) | 1`.
fn report_to_tohost(encode: Instruction, tohost: u64, xlen: Xlen) -> Vec<Instruction> {
    let mut instrs = vec![
        encode,
        Instruction::Ori {
            rd: XReg::X5,
            rs1: XReg::X5,
            imm: 1,
        },
    ];
    instrs.extend(load_imm(XReg::X6, tohost as i64, xlen));
    // Order all prior memory accesses before the exit command.
    instrs.push(Instruction::Fence { imm: 0x33 });
    if xlen == Xlen::X32 {
        // tohost is 64 bits wide; clear the upper word before the command.
        instrs.push(Instruction::Sw {
            rs1: XReg::X6,
            rs2: XReg::ZERO,
            imm: 4,
        });
        instrs.push(Instruction::Sw {
            rs1: XReg::X6,
            rs2: XReg::X5,
            imm: 0,
        });
    } else {
        instrs.push(Instruction::Sd {
            rs1: XReg::X6,
            rs2: XReg::X5,
            imm: 0,
        });
    }
    instrs.push(Instruction::Jal {
        rd: XReg::ZERO,
        imm: 0,
    });
    instrs
}

/// A random XLEN-wide value, sign-extended to i64 for [`load_imm`].
fn random_xlen(rng: &mut (impl Rng + ?Sized), xlen: Xlen) -> i64 {
    match xlen {
        Xlen::X32 => rng.random::<i32>().into(),
        Xlen::X64 => rng.random::<i64>(),
    }
}

/// A random `align`-aligned start in `lo..=hi` for `len` bytes that stay in
/// `start..end` and off the `used` ranges, if any.
fn free_spot(
    used: &BTreeMap<u64, u64>,
    start: u64,
    end: u64,
    lo: u64,
    hi: u64,
    len: u64,
    align: u64,
    rng: &mut (impl Rng + ?Sized),
) -> Option<u64> {
    // The valid starts in each gap, as (first, count); then a uniform pick
    // among all of them, so big gaps are not crowded out by small ones.
    let mut spans = Vec::new();
    let mut gap = start;
    for (&from, &to) in used.iter().chain(std::iter::once((&end, &end))) {
        let first = gap.max(lo).next_multiple_of(align);
        let last = from.saturating_sub(len).min(hi);
        if first <= last {
            spans.push((first, (last - first) / align + 1));
        }
        gap = to;
    }
    let total: u64 = spans.iter().map(|(_, count)| count).sum();
    if total == 0 {
        return None;
    }
    let mut pick = rng.random_range(0..total);
    for (first, count) in spans {
        if pick < count {
            return Some(first + pick * align);
        }
        pick -= count;
    }
    unreachable!("pick < total")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{SeedableRng, rngs::StdRng};

    #[test]
    fn free_spot_stays_in_reach_and_off_used_ranges() {
        let mut rng = StdRng::seed_from_u64(0);
        let used = BTreeMap::from([(0x100, 0x180), (0x200, 0x300)]);
        for _ in 0..1000 {
            let at = free_spot(&used, 0, 0x400, 0xf0, 0x2a0, 0x40, 2, &mut rng).unwrap();
            assert!((0xf0..=0x2a0).contains(&at) && at % 2 == 0);
            assert!(used.iter().all(|(&from, &to)| at + 0x40 <= from || at >= to));
        }
        // The only gap in reach, 0x180..0x200, is a byte too small.
        assert_eq!(free_spot(&used, 0, 0x400, 0x101, 0x1c0, 0x81, 2, &mut rng), None);
    }

    #[test]
    fn scattered_programs_place_and_encode() {
        use crate::options::CommonOpts;
        use crate::riscv::Extension::{C, I, M, Zicsr};
        for seed in 0..20 {
            let mut opts = CommonOpts::default();
            opts.isa = vec![I, M, C, Zicsr];
            opts.num_instrs = 4096;
            opts.no_self_check = true;
            opts.seed = Some(seed);
            crate::orchestrator::generate(&opts).unwrap();
        }
    }
}
