//! Data/control-flow entanglement, after Cascade.
//!
//! Sites make the program's path depend on register values computed by the
//! workload, so a core that miscomputes one diverges instead of carrying on
//! silently:
//!
//! - A *value* site builds a register as `r = golden; r ^= dep_1; ...;
//!   r ^= dep_k`, where the `dep_i` hold workload data and the *golden value*
//!   is patched to `dest ^ dep_1 ^ ... ^ dep_k` once Spike, the golden model,
//!   tells us their values. It serves as an indirect jump target, or (with
//!   `dest = 0`) the accumulator of a guard. On RV64 the golden value is
//!   loaded from the `.golden` data section (a [`PoolRef`]), so patching it
//!   rewrites data rather than code; on RV32, or with `--inline-golden`, it is
//!   materialized inline with [`load_imm_fixed`], which is as short there.
//! - A *branch* site compares two workload registers. Its opcode is picked
//!   once Spike tells us their values, so the planned direction is the right
//!   one. A taken branch ends its block and jumps to the next one, falling
//!   through to a jump to `_fail`; a not-taken one targets a random decoy
//!   nearby, so a core that wrongly takes it runs junk.
//! - A *guard* tests a value site's accumulator against zero with `beq`/`bne`
//!   (or `c.beqz`/`c.bnez`). Unlike the others, which only notice faults that
//!   move a jump target or a comparison far enough, it notices any wrong value
//!   among its `dep_i`.
//! - A *jump* site ends a block with `jal rd` or `c.j` to the next block.
//! - A *link* site points `rd` at the next block (`auipc; addi`), so a `ret`
//!   lands there.
//!
//! Branch and jump sites only peek at workload registers; guards consume
//! them, so every register the workload writes is checked by the next guard
//! unless the workload overwrites it first. Memory accesses are not sites:
//! they go through reserved base registers (see [`crate::membase`]), which
//! site code never writes.
//!
//! Sites have fixed sizes, so patching moves no code. Before patching, a site
//! renders a placeholder that follows the planned path without depending on
//! the workload's values (the draft Spike runs); the register state at every
//! site is the same in the draft and in the final program.
use anyhow::{Result, anyhow, ensure};
use rand::distr::{Distribution, weighted::WeightedIndex};
use rand::{Rng, RngExt};

use crate::riscv::asmutil::{load_imm_fixed, split_imm32, to_unsigned, twos_complement};
use crate::riscv::{EncodedInstruction, Instruction, Opcode, XReg, Xlen};
use crate::target::Target;
use crate::weights::InstrWeights;

/// Branch opcodes as complementary pairs: for any operands, exactly one of each
/// pair is taken, so an enabled pair can always realize either direction.
const BRANCH_PAIRS: [(Opcode, Opcode); 3] = [
    (Opcode::Beq, Opcode::Bne),
    (Opcode::Blt, Opcode::Bge),
    (Opcode::Bltu, Opcode::Bgeu),
];

/// Most workload registers a jump target is entangled with.
const MAX_PEEK: usize = 4;

/// Bytes per constant in the `.golden` section.
pub const GOLDEN_SLOT_SIZE: u64 = 8;

/// Instructions loading a golden value from `.golden`: `auipc r; ld r`.
const GOLDEN_LOAD_LEN: usize = 2;

/// A PC-relative reference to slot `slot` of `.golden` at index `at` of a
/// block: `auipc rd; ld rd` loads the slot, or with `load` false,
/// `auipc rd; addi rd` points `rd` at it. Written once the code is linked.
#[derive(Debug, Clone, Copy)]
pub struct PoolRef {
    pub at: usize,
    pub rd: XReg,
    pub slot: usize,
    pub load: bool,
}

impl PoolRef {
    /// The two instructions for a slot `offset` bytes from the `auipc`.
    pub fn instrs(&self, offset: i32) -> [Instruction; 2] {
        let (hi, lo) = split_imm32(offset as u32);
        let rd = self.rd;
        [
            Instruction::Auipc { rd, imm: hi },
            if self.load {
                Instruction::Ld { rd, rs1: rd, imm: lo }
            } else {
                Instruction::Addi { rd, rs1: rd, imm: lo }
            },
        ]
    }

    /// Write the reference into `instrs`, the block starting at `block_addr`,
    /// given that `.golden` starts at `golden_addr`.
    pub fn link(&self, instrs: &mut [Instruction], block_addr: u64, golden_addr: Option<u64>) -> Result<()> {
        let golden_addr = golden_addr.ok_or_else(|| anyhow!("code reads .golden, which is not reserved"))?;
        let pc = block_addr + code_size(&instrs[..self.at]) as u64;
        let offset = (golden_addr + self.slot as u64 * GOLDEN_SLOT_SIZE).wrapping_sub(pc) as i64;
        ensure!(offset == offset as i32 as i64, ".golden is out of AUIPC range");
        instrs[self.at..self.at + 2].copy_from_slice(&self.instrs(offset as i32));
        Ok(())
    }
}

/// Where a value site's register must end up pointing.
#[derive(Debug, Clone, Copy)]
pub enum Dest {
    /// An absolute value: 0 for a guard.
    Data(u64),
    /// The first instruction of the next block in execution order. Resolved
    /// to an address by [`Site::link`].
    Next,
}

#[derive(Debug, Clone)]
pub enum Kind {
    Value {
        rprod: XReg,
        /// Never contains `rprod`.
        rdeps: Vec<XReg>,
        dest: Dest,
        /// Absolute value of `dest`, known once the code is linked.
        value: Option<u64>,
        /// XOR of Spike's values of `rdeps` at the site, known once resolved.
        rdep_val: Option<u64>,
        /// Index of the `.golden` slot holding the golden value, or `None`
        /// when it is materialized inline.
        golden_slot: Option<usize>,
    },
    Branch {
        rs1: XReg,
        rs2: XReg,
        taken: bool,
        /// Weights of the enabled branch opcodes, which bias the choice of
        /// `opcode`.
        weights: Vec<(Opcode, f64)>,
        /// Chosen once Spike tells us the operands' values.
        opcode: Option<Opcode>,
        /// Byte offset of the target: the next block when `taken` (set by
        /// [`Site::link`]), a random decoy otherwise.
        offset: i32,
        /// `c.beqz`/`c.bnez rs1` (`rs2` is x0) instead of a base branch.
        compressed: bool,
    },
    /// `beq reg, x0` when `taken`, `bne reg, x0` otherwise (`c.beqz`/`c.bnez`
    /// if `compressed`); `reg` is 0 in a correct execution.
    Guard { reg: XReg, taken: bool, compressed: bool },
    /// `jal rd, next`, or `c.j next` if `compressed`; `offset` is set by
    /// [`Site::link`].
    Jump { rd: XReg, compressed: bool, offset: i32 },
    /// `auipc rd; addi rd`, pointing `rd` at the next block; `offset` is set
    /// by [`Site::link`].
    Link { rd: XReg, offset: i32 },
}

/// One entanglement site inside a basic block's instructions.
#[derive(Debug, Clone)]
pub struct Site {
    /// Index of the site's first instruction in the block.
    pub at: usize,
    pub kind: Kind,
    /// Index of the `jal x0, _fail` guarding the site, if any.
    pub fail_at: Option<usize>,
}

impl Site {
    /// Number of instructions in the producer sequence of a value site:
    /// a load from `.golden` if `golden_slot`, an inline constant otherwise.
    fn producer_len(xlen: Xlen, golden_slot: bool) -> usize {
        if golden_slot {
            GOLDEN_LOAD_LEN
        } else {
            load_imm_fixed(XReg::ZERO, 0, xlen).len()
        }
    }

    /// Index of the instruction at which Spike must report the registers the
    /// site depends on, if it depends on any: the first XOR of a value site,
    /// the branch itself for a branch site.
    pub fn probe_index(&self, xlen: Xlen) -> Option<usize> {
        match self.kind {
            Kind::Value { golden_slot, .. } => {
                Some(self.at + Self::producer_len(xlen, golden_slot.is_some()))
            }
            Kind::Branch { .. } => Some(self.at),
            Kind::Guard { .. } | Kind::Jump { .. } | Kind::Link { .. } => None,
        }
    }

    /// Whether the site leaves its block for the next one.
    pub fn ends_block(&self) -> bool {
        matches!(
            self.kind,
            Kind::Branch { taken: true, .. } | Kind::Jump { .. } | Kind::Value { dest: Dest::Next, .. }
        )
    }

    /// How far a PC-relative jump to the next block reaches: its offset must
    /// lie in `-reach..reach`. `None` if the site jumps anywhere or not at all.
    pub fn reach(&self) -> Option<u64> {
        match self.kind {
            Kind::Branch { taken: true, compressed: true, .. } => Some(1 << 8),
            Kind::Branch { taken: true, .. } => Some(1 << 12),
            Kind::Jump { compressed: true, .. } => Some(1 << 11),
            Kind::Jump { .. } => Some(1 << 20),
            _ => None,
        }
    }

    /// Fix code addresses: `block_addr` is the absolute address of the
    /// block's first instruction, `next_addr` that of the next block in
    /// execution order, `fail_addr` that of `_fail` and `golden_addr` that of
    /// the `.golden` section, if any. A site reading `.golden` gets its
    /// `auipc; ld` here, as its address never changes.
    pub fn link(
        &mut self,
        instrs: &mut [Instruction],
        block_addr: u64,
        next_addr: Option<u64>,
        fail_addr: Option<u64>,
        golden_addr: Option<u64>,
    ) -> Result<()> {
        let addr_of = |instrs: &[Instruction], index: usize| {
            block_addr + code_size(&instrs[..index]) as u64
        };
        if let Some(fail_at) = self.fail_at {
            let fail_addr = fail_addr.ok_or_else(|| anyhow!("site jumps to _fail, which is missing"))?;
            let offset = fail_addr.wrapping_sub(addr_of(instrs, fail_at)) as i64;
            ensure!(
                (-(1 << 20)..(1 << 20)).contains(&offset),
                "_fail is out of JAL range"
            );
            instrs[fail_at] = Instruction::Jal {
                rd: XReg::ZERO,
                imm: offset as i32,
            };
        }
        if let Kind::Value {
            rprod,
            golden_slot: Some(slot),
            ..
        } = self.kind
        {
            let load = PoolRef { at: self.at, rd: rprod, slot, load: true };
            load.link(instrs, block_addr, golden_addr)?;
        }
        let next = || next_addr.ok_or_else(|| anyhow!("site jumps to the next block, which is missing"));
        let pc = addr_of(instrs, self.at);
        match &mut self.kind {
            Kind::Value { dest, value, .. } => {
                *value = Some(match *dest {
                    Dest::Data(value) => value,
                    Dest::Next => next()?,
                });
            }
            Kind::Branch { taken: true, offset, .. } | Kind::Jump { offset, .. } | Kind::Link { offset, .. } => {
                *offset = i32::try_from(next()?.wrapping_sub(pc) as i64)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Record the register values Spike reported at the probe and pick what
    /// depends on them.
    pub fn resolve(
        &mut self,
        xregs: &[u64; 32],
        target: &Target,
        rng: &mut (impl Rng + ?Sized),
    ) -> Result<()> {
        let xlen = target.xlen;
        match &mut self.kind {
            Kind::Value { rdeps, rdep_val, .. } => {
                *rdep_val = Some(rdeps.iter().fold(0, |acc, reg| acc ^ xregs[reg.index() as usize]));
            }
            Kind::Branch {
                rs1,
                rs2,
                taken,
                weights,
                opcode,
                ..
            } => {
                let a = xregs[rs1.index() as usize];
                let b = xregs[rs2.index() as usize];
                // The data decides which opcodes realize the planned direction;
                // the weights choose among them, uniformly if they are all 0.
                let candidates: Vec<_> = weights
                    .iter()
                    .filter(|(op, _)| branch_taken(*op, a, b, xlen) == *taken)
                    .collect();
                ensure!(!candidates.is_empty(), "no enabled branch realizes the planned direction");
                let chosen = match WeightedIndex::new(candidates.iter().map(|(_, w)| *w)) {
                    Ok(index) => candidates[index.sample(rng)],
                    Err(_) => candidates[rng.random_range(0..candidates.len())],
                };
                *opcode = Some(chosen.0);
            }
            Kind::Guard { .. } | Kind::Jump { .. } | Kind::Link { .. } => {}
        }
        Ok(())
    }

    /// The golden value: what the producer must load so that XORing the
    /// `rdeps` yields `dest`. `value` in the draft, where the XORs read x0;
    /// `value ^ rdeps` once Spike has reported the `rdeps`.
    fn golden(&self) -> Result<Option<u64>> {
        let Kind::Value { value, rdep_val, .. } = self.kind else {
            return Ok(None);
        };
        let value = value.ok_or_else(|| anyhow!("site rendered before linking"))?;
        Ok(Some(value ^ rdep_val.unwrap_or(0)))
    }

    /// The `.golden` slot this site reads and the golden value it must hold,
    /// once linking has fixed the site's destination.
    pub fn golden_entry(&self) -> Option<(usize, u64)> {
        match self.kind {
            Kind::Value {
                golden_slot: Some(slot),
                value: Some(value),
                rdep_val,
                ..
            } => Some((slot, value ^ rdep_val.unwrap_or(0))),
            _ => None,
        }
    }

    /// Write the site's instructions for what is currently known: the
    /// placeholder before [`Site::resolve`], the entangled form after.
    pub fn render(&self, instrs: &mut [Instruction], xlen: Xlen) -> Result<()> {
        match self.kind {
            Kind::Value {
                rprod,
                ref rdeps,
                rdep_val,
                golden_slot,
                ..
            } => {
                // Draft: rprod = value ^ 0 ^ ... ^ 0.
                // Final: rprod = (value ^ rdeps) ^ rdep_1 ^ ... ^ rdep_k.
                let golden = self.golden()?.expect("a value site");
                let n = Self::producer_len(xlen, golden_slot.is_some());
                // A load from .golden is written by `link`; the golden value
                // itself goes into the section (see `golden_entry`).
                if golden_slot.is_none() {
                    let producer = load_imm_fixed(rprod, golden as i64, xlen);
                    instrs[self.at..self.at + n].copy_from_slice(&producer);
                }
                for (i, rdep) in rdeps.iter().enumerate() {
                    instrs[self.at + n + i] = Instruction::Xor {
                        rd: rprod,
                        rs1: rprod,
                        rs2: if rdep_val.is_some() { *rdep } else { XReg::ZERO },
                    };
                }
            }
            Kind::Branch {
                rs1,
                rs2,
                taken,
                opcode,
                offset,
                compressed,
                ..
            } => {
                instrs[self.at] = match (opcode, compressed) {
                    (Some(op), _) => branch(op, rs1, rs2, offset)?,
                    (None, false) if taken => Instruction::Jal { rd: XReg::ZERO, imm: offset },
                    (None, true) if taken => Instruction::CJ { imm: offset },
                    (None, false) => Instruction::nop(),
                    (None, true) => Instruction::cnop(),
                };
            }
            // A guard skips the `jal` after it when taken.
            Kind::Guard { reg, taken, compressed } => {
                instrs[self.at] = match (taken, compressed) {
                    (true, false) => branch(Opcode::Beq, reg, XReg::ZERO, 8)?,
                    (false, false) => branch(Opcode::Bne, reg, XReg::ZERO, 8)?,
                    (true, true) => Instruction::CBeqz { rs1: reg, imm: 6 },
                    (false, true) => Instruction::CBnez { rs1: reg, imm: 6 },
                };
            }
            Kind::Jump { rd, compressed, offset } => {
                instrs[self.at] = if compressed {
                    Instruction::CJ { imm: offset }
                } else {
                    Instruction::Jal { rd, imm: offset }
                };
            }
            Kind::Link { rd, offset } => {
                let (hi, lo) = split_imm32(offset as u32);
                instrs[self.at] = Instruction::Auipc { rd, imm: hi };
                instrs[self.at + 1] = Instruction::Addi { rd, rs1: rd, imm: lo };
            }
        }
        Ok(())
    }
}

/// Builds entanglement sites across a hart's workload.
///
/// A fault is only caught if the corrupted register reaches a site before
/// the workload overwrites it, so sites use *fresh* registers: those written
/// by the workload since the last guard, most recent first. Uniformly random
/// operands catch far fewer faults.
pub struct SiteBuilder<'a> {
    target: &'a Target,
    /// Workload-written registers not yet consumed by a guard, oldest first.
    fresh: Vec<XReg>,
    /// A guard is also placed mid-block once this many registers are fresh
    /// (never if 0), so fewer faulty values are overwritten before a guard
    /// sees them. Lower catches more faults but adds more generator code.
    guard_threshold: usize,
    /// Registers site code must never write or read (the memory bases).
    reserved: Vec<XReg>,
    /// Whether value sites load their golden value from `.golden`.
    golden_section: bool,
    /// `.golden` slots handed out so far.
    golden_slots: usize,
}

impl<'a> SiteBuilder<'a> {
    /// With `golden_section` (RV64 only), value sites load their golden value
    /// from the `.golden` section.
    pub fn new(target: &'a Target, guard_threshold: usize, reserved: Vec<XReg>, golden_section: bool) -> Self {
        Self {
            target,
            fresh: Vec::new(),
            guard_threshold,
            reserved,
            golden_section,
            golden_slots: 0,
        }
    }

    /// Number of `.golden` slots the sites built so far read.
    pub fn golden_slots(&self) -> usize {
        self.golden_slots
    }

    /// The register the workload wrote most recently, if still unchecked.
    pub fn freshest(&self) -> Option<XReg> {
        self.fresh.last().copied()
    }

    /// Record that the workload instruction `instr` was appended.
    pub fn observe(&mut self, instr: &Instruction) {
        if let Some(rd) = written_xreg(instr) {
            self.fresh.retain(|reg| *reg != rd);
            self.fresh.push(rd);
        }
    }

    /// Record that generator code overwrote `reg` with a known value, so it
    /// no longer carries workload data.
    pub fn clobber(&mut self, reg: XReg) {
        self.fresh.retain(|r| *r != reg);
    }

    /// After a workload instruction: a guard if enough registers are fresh.
    pub fn after_instr(&mut self, instrs: &mut Vec<Instruction>, rng: &mut (impl Rng + ?Sized)) -> Vec<Site> {
        if self.can_guard() && self.guard_threshold > 0 && self.fresh.len() >= self.guard_threshold {
            self.guard(instrs, rng)
        } else {
            Vec::new()
        }
    }

    /// Append the site ending a block: a guard over every fresh register.
    pub fn block_end(&mut self, instrs: &mut Vec<Instruction>, rng: &mut (impl Rng + ?Sized)) -> Vec<Site> {
        if self.can_guard() && !self.fresh.is_empty() {
            self.guard(instrs, rng)
        } else {
            Vec::new()
        }
    }

    /// Whether [`SiteBuilder::control_flow`] can generate `opcode`: it is
    /// enabled and, for a branch, some complementary pair is too, so either
    /// direction can be realized.
    pub fn can_generate(&self, opcode: Opcode) -> bool {
        match opcode {
            Opcode::Jalr | Opcode::CJr | Opcode::CJalr | Opcode::CJ => self.enabled(opcode),
            Opcode::CBeqz | Opcode::CBnez => self.enabled(Opcode::CBeqz) && self.enabled(Opcode::CBnez),
            _ => {
                BRANCH_PAIRS.iter().any(|(a, b)| opcode == *a || opcode == *b)
                    && self.enabled(opcode)
                    && self.can_branch()
            }
        }
    }

    fn can_branch(&self) -> bool {
        BRANCH_PAIRS
            .iter()
            .any(|(a, b)| self.enabled(*a) && self.enabled(*b))
    }

    /// A control-flow site for a drawn `opcode`: for `jalr`, `c.jr` and
    /// `c.jalr`, an indirect jump to the next block, through `ra`/`t0` half of
    /// the time so it pushes or pops the return-address stack; for `c.j`, a
    /// jump to the next block; otherwise a conditional branch, which ends the
    /// block if taken. The concrete branch opcode is picked later from
    /// `weights`, among the base ones or `c.beqz`/`c.bnez`.
    pub fn control_flow(
        &mut self,
        instrs: &mut Vec<Instruction>,
        opcode: Opcode,
        weights: &InstrWeights,
        rng: &mut (impl Rng + ?Sized),
    ) -> Site {
        debug_assert!(self.can_generate(opcode), "{opcode} cannot be generated");
        if opcode == Opcode::CJ {
            let at = instrs.len();
            instrs.push(Instruction::cnop());
            return Site { at, kind: Kind::Jump { rd: XReg::ZERO, compressed: true, offset: 0 }, fail_at: None };
        }
        if matches!(opcode, Opcode::Jalr | Opcode::CJr | Opcode::CJalr) {
            let rprod = self.link_or_stale(rng);
            let rd = match opcode {
                Opcode::CJr => XReg::ZERO,
                Opcode::CJalr => XReg::RA,
                _ if rng.random_bool(1.0 / 3.0) => XReg::ZERO,
                _ => self.link_or_stale(rng),
            };
            let rdeps = self.peek(rng, MAX_PEEK, rprod);
            self.clobber(rprod);
            self.clobber(rd);
            // producer; xor...; jalr rd, 0(rprod); jal x0, _fail
            let mut site = self.value_site(instrs, rprod, rdeps, Dest::Next);
            instrs.push(match opcode {
                Opcode::CJr => Instruction::CJr { rs1: rprod },
                Opcode::CJalr => Instruction::CJalr { rs1: rprod },
                _ => Instruction::Jalr { rd, rs1: rprod, imm: 0 },
            });
            site.fail_at = Some(instrs.len());
            instrs.push(Instruction::nop());
            return site;
        }
        // c.beqz/c.bnez compare one of x8..x15, the freshest if any, with zero.
        let compressed = matches!(opcode, Opcode::CBeqz | Opcode::CBnez);
        let (rs1, rs2, opcodes): (_, _, Vec<_>) = if compressed {
            let rs1 = self.fresh.iter().rev().copied().find(|reg| (8..=15).contains(&reg.index()));
            let rs1 = rs1.unwrap_or_else(|| XReg::new(rng.random_range(8..=15)).expect("valid register"));
            (rs1, XReg::ZERO, vec![Opcode::CBeqz, Opcode::CBnez])
        } else {
            let operands = self.peek(rng, 2, XReg::ZERO);
            let rs1 = operands[0];
            let rs2 = match operands.get(1) {
                Some(reg) => *reg,
                None => self.random_reg_except(rng, rs1),
            };
            (rs1, rs2, enabled_branch_opcodes(self.target).collect())
        };
        let taken = rng.random_bool(0.5);
        let weights = opcodes.into_iter().map(|op| (op, weights.get(op))).collect();
        // taken: b<cc> next; jal _fail. Not taken: b<cc> decoy.
        let at = instrs.len();
        instrs.push(if compressed { Instruction::cnop() } else { Instruction::nop() });
        let fail_at = taken.then(|| {
            instrs.push(Instruction::nop());
            at + 1
        });
        let offset = if taken { 0 } else { self.decoy(rng, if compressed { 256 } else { 4096 }) };
        Site {
            at,
            kind: Kind::Branch { rs1, rs2, taken, weights, opcode: None, offset, compressed },
            fail_at,
        }
    }

    /// A random branch offset within `reach` bytes other than the
    /// fall-through, mostly landing in the random bytes between blocks.
    fn decoy(&self, rng: &mut (impl Rng + ?Sized), reach: i32) -> i32 {
        let align = self.target.instruction_alignment() as i32;
        loop {
            let offset = rng.random_range(-reach / align..reach / align) * align;
            if !(0..=4).contains(&offset) {
                return offset;
            }
        }
    }

    /// `acc = K ^ fresh_1 ^ ... ^ fresh_k`, which is 0 in a correct execution,
    /// then a branch to `_fail` unless it is. Consumes every fresh register.
    fn guard(&mut self, instrs: &mut Vec<Instruction>, rng: &mut (impl Rng + ?Sized)) -> Vec<Site> {
        // c.beqz/c.bnez only take x8..x15.
        let compact: Vec<_> = self.stale_regs().into_iter().filter(|reg| (8..=15).contains(&reg.index())).collect();
        let compressed =
            !compact.is_empty() && use_compressed(self.target, &[Opcode::CBeqz, Opcode::CBnez], rng);
        let acc = if compressed { compact[rng.random_range(0..compact.len())] } else { self.stale(rng) };
        let rdeps: Vec<_> = self.fresh.drain(..).filter(|reg| *reg != acc).collect();
        let value = self.value_site(instrs, acc, rdeps, Dest::Data(0));
        let taken = rng.random_bool(0.5);
        let guard = guard_site(instrs, acc, taken, compressed);
        vec![value, guard]
    }

    fn value_site(&mut self, instrs: &mut Vec<Instruction>, rprod: XReg, rdeps: Vec<XReg>, dest: Dest) -> Site {
        let at = instrs.len();
        // Placeholders; the site renders its real instructions once linked.
        let n = Site::producer_len(self.target.xlen, self.golden_section) + rdeps.len();
        instrs.extend(std::iter::repeat_n(Instruction::nop(), n));
        let golden_slot = self.golden_section.then(|| {
            self.golden_slots += 1;
            self.golden_slots - 1
        });
        Site {
            at,
            kind: Kind::Value {
                rprod,
                rdeps,
                dest,
                value: None,
                rdep_val: None,
                golden_slot,
            },
            fail_at: None,
        }
    }

    /// Up to `max` of the freshest registers other than `except`, left fresh;
    /// a random nonzero register if none is fresh.
    fn peek(&self, rng: &mut (impl Rng + ?Sized), max: usize, except: XReg) -> Vec<XReg> {
        let regs: Vec<_> = self
            .fresh
            .iter()
            .rev()
            .copied()
            .filter(|reg| *reg != except)
            .take(max)
            .collect();
        if regs.is_empty() {
            vec![self.random_reg_except(rng, except)]
        } else {
            regs
        }
    }

    /// A random nonzero, unreserved register holding no unchecked workload
    /// value, for generator code to overwrite; any unreserved one if all are
    /// fresh.
    fn stale(&self, rng: &mut (impl Rng + ?Sized)) -> XReg {
        let stale = self.stale_regs();
        if stale.is_empty() {
            self.random_reg_except(rng, XReg::ZERO)
        } else {
            stale[rng.random_range(0..stale.len())]
        }
    }

    fn stale_regs(&self) -> Vec<XReg> {
        (1..32)
            .map(|i| XReg::new(i).expect("valid register"))
            .filter(|reg| !self.fresh.contains(reg) && !self.reserved.contains(reg))
            .collect()
    }

    /// `ra` or `t0` half of the time when one is stale, else [`Self::stale`].
    fn link_or_stale(&self, rng: &mut (impl Rng + ?Sized)) -> XReg {
        let links: Vec<_> = [XReg::RA, XReg::T0]
            .into_iter()
            .filter(|reg| !self.fresh.contains(reg) && !self.reserved.contains(reg))
            .collect();
        if !links.is_empty() && rng.random_bool(0.5) {
            links[rng.random_range(0..links.len())]
        } else {
            self.stale(rng)
        }
    }

    /// A random nonzero, unreserved register other than `except`.
    fn random_reg_except(&self, rng: &mut (impl Rng + ?Sized), except: XReg) -> XReg {
        loop {
            let reg = XReg::new(rng.random_range(1..32)).expect("valid register");
            if reg != except && !self.reserved.contains(&reg) {
                return reg;
            }
        }
    }

    fn can_guard(&self) -> bool {
        self.enabled(Opcode::Beq) && self.enabled(Opcode::Bne)
    }

    fn enabled(&self, opcode: Opcode) -> bool {
        self.target.supports(opcode) && !self.target.disabled_opcodes.contains(&opcode)
    }
}

/// Whether to use the compressed form, which needs all of `opcodes`: half of
/// the time when they are enabled.
pub fn use_compressed(target: &Target, opcodes: &[Opcode], rng: &mut (impl Rng + ?Sized)) -> bool {
    opcodes
        .iter()
        .all(|op| target.supports(*op) && !target.disabled_opcodes.contains(op))
        && rng.random_bool(0.5)
}

/// Append a guard, which skips the next instruction when taken:
///   taken:     b<cc> skip; jal _fail
///   not taken: b<cc> skip; jal +8; jal _fail
fn guard_site(instrs: &mut Vec<Instruction>, reg: XReg, taken: bool, compressed: bool) -> Site {
    let at = instrs.len();
    instrs.push(if compressed { Instruction::cnop() } else { Instruction::nop() });
    if !taken {
        instrs.push(Instruction::Jal {
            rd: XReg::ZERO,
            imm: 8,
        });
    }
    let fail_at = instrs.len();
    instrs.push(Instruction::nop());
    Site {
        at,
        kind: Kind::Guard { reg, taken, compressed },
        fail_at: Some(fail_at),
    }
}

fn enabled_branch_opcodes(target: &Target) -> impl Iterator<Item = Opcode> + '_ {
    BRANCH_PAIRS
        .iter()
        .flat_map(|(a, b)| [*a, *b])
        .filter(|op| target.supports(*op) && !target.disabled_opcodes.contains(op))
}

/// The integer register `instr` writes, if any. Decoded from the standard
/// encoding; compressed instructions are not tracked.
pub fn written_xreg(instr: &Instruction) -> Option<XReg> {
    let Ok(EncodedInstruction::Standard(bits)) = instr.encode() else {
        return None;
    };
    let (opcode, funct3, funct7) = (bits & 0x7f, (bits >> 12) & 7, bits >> 25);
    let writes = match opcode {
        // load, op-imm, auipc, op-imm-32, amo, op, lui, op-32
        0x03 | 0x13 | 0x17 | 0x1b | 0x2f | 0x33 | 0x37 | 0x3b => true,
        // CSR accesses (funct3 0 is ecall/ebreak/xret/wfi/fences)
        0x73 => funct3 != 0,
        // FP ops with an integer result: fcmp, fcvt.int.fp, fmv.x/fclass
        0x53 => matches!(funct7, 0x50 | 0x51 | 0x60 | 0x61 | 0x70 | 0x71),
        _ => false,
    };
    let rd = XReg::new(((bits >> 7) & 31) as u8).ok()?;
    (writes && rd != XReg::ZERO).then_some(rd)
}

fn branch_taken(opcode: Opcode, a: u64, b: u64, xlen: Xlen) -> bool {
    let (sa, sb) = (twos_complement(a, xlen), twos_complement(b, xlen));
    let (ua, ub) = (to_unsigned(sa, xlen), to_unsigned(sb, xlen));
    match opcode {
        Opcode::Beq => ua == ub,
        Opcode::Bne => ua != ub,
        Opcode::Blt => sa < sb,
        Opcode::Bge => sa >= sb,
        Opcode::Bltu => ua < ub,
        Opcode::Bgeu => ua >= ub,
        // Their rs2 is x0, so `b` is 0.
        Opcode::CBeqz => ua == ub,
        Opcode::CBnez => ua != ub,
        _ => unreachable!("not a branch: {opcode}"),
    }
}

fn branch(opcode: Opcode, rs1: XReg, rs2: XReg, imm: i32) -> Result<Instruction> {
    use Instruction::*;
    Ok(match opcode {
        Opcode::Beq => Beq { rs1, rs2, imm },
        Opcode::Bne => Bne { rs1, rs2, imm },
        Opcode::Blt => Blt { rs1, rs2, imm },
        Opcode::Bge => Bge { rs1, rs2, imm },
        Opcode::Bltu => Bltu { rs1, rs2, imm },
        Opcode::Bgeu => Bgeu { rs1, rs2, imm },
        Opcode::CBeqz => CBeqz { rs1, imm },
        Opcode::CBnez => CBnez { rs1, imm },
        _ => return Err(anyhow!("not a branch: {opcode}")),
    })
}

pub fn code_size(instrs: &[Instruction]) -> usize {
    instrs.iter().map(Instruction::byte_len).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_operand_pair_takes_one_of_each_pair() {
        for xlen in [Xlen::X32, Xlen::X64] {
            for (a, b) in [(0, 0), (1, 2), (u64::MAX, 1), (0x8000_0000, 0x7fff_ffff)] {
                for (p, q) in BRANCH_PAIRS {
                    assert_ne!(branch_taken(p, a, b, xlen), branch_taken(q, a, b, xlen));
                }
            }
        }
        // 0x8000_0000 is negative on RV32 only.
        assert!(branch_taken(Opcode::Blt, 0x8000_0000, 0, Xlen::X32));
        assert!(!branch_taken(Opcode::Blt, 0x8000_0000, 0, Xlen::X64));
    }

    /// An RV64I machine-mode target.
    fn test_target() -> Target {
        use crate::memory::{MemoryRegion, Permissions};
        use crate::riscv::Extension;
        Target::new(
            Xlen::X64,
            [Extension::I, Extension::Zicsr],
            [crate::riscv::PrivilegeLevel::Machine],
            Default::default(),
            1,
            10,
            MemoryRegion {
                start: 0x8000_0000,
                size: 0x1000,
                permissions: Permissions::default(),
            },
            Some(0),
            0,
            0,
        )
        .unwrap()
    }

    fn resolved_branch(weights: &[(Opcode, f64)], taken: bool, seed: u64) -> Opcode {
        use rand::SeedableRng;
        let target = test_target();
        let mut site = Site {
            at: 0,
            kind: Kind::Branch {
                rs1: XReg::A0,
                rs2: XReg::A1,
                taken,
                weights: weights.to_vec(),
                opcode: None,
                offset: 8,
                compressed: false,
            },
            fail_at: None,
        };
        // a0 = a1 = 5 and a0 < a1 is false: beq, bge, bgeu are taken.
        let mut xregs = [0u64; 32];
        xregs[XReg::A0.index() as usize] = 5;
        xregs[XReg::A1.index() as usize] = 5;
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        site.resolve(&xregs, &target, &mut rng).unwrap();
        match site.kind {
            Kind::Branch { opcode: Some(opcode), .. } => opcode,
            _ => unreachable!(),
        }
    }

    #[test]
    fn branch_opcode_follows_weights_among_those_realizing_the_direction() {
        let all = |beq, bne, blt, bge, bltu, bgeu| {
            vec![
                (Opcode::Beq, beq),
                (Opcode::Bne, bne),
                (Opcode::Blt, blt),
                (Opcode::Bge, bge),
                (Opcode::Bltu, bltu),
                (Opcode::Bgeu, bgeu),
            ]
        };
        for seed in 0..50 {
            // Taken: only beq, bge, bgeu qualify; the weights pick bge.
            assert_eq!(resolved_branch(&all(0.0, 9.0, 9.0, 1.0, 9.0, 0.0), true, seed), Opcode::Bge);
            // Not taken: only bne, blt, bltu qualify; the weights pick bltu.
            assert_eq!(resolved_branch(&all(9.0, 0.0, 0.0, 9.0, 3.0, 9.0), false, seed), Opcode::Bltu);
            // No qualifying opcode has weight: any qualifying one will do.
            let any = resolved_branch(&all(0.0, 9.0, 9.0, 0.0, 9.0, 0.0), true, seed);
            assert!(matches!(any, Opcode::Beq | Opcode::Bge | Opcode::Bgeu));
        }
    }

    #[test]
    fn golden_value_is_loaded_from_its_slot() {
        use rand::SeedableRng;
        // A guard at 0x8000_0010 reading slot 3 of .golden at 0x87ff_f000.
        let (block, golden_addr) = (0x8000_0000u64, 0x87ff_f000u64);
        let mut instrs = vec![Instruction::nop(); 8];
        let mut site = Site {
            at: 4,
            kind: Kind::Value {
                rprod: XReg::A0,
                rdeps: vec![XReg::A1, XReg::A2],
                dest: Dest::Data(0),
                value: None,
                rdep_val: None,
                golden_slot: Some(3),
            },
            fail_at: None,
        };
        site.link(&mut instrs, block, None, Some(block + 0x100), Some(golden_addr)).unwrap();
        site.render(&mut instrs, Xlen::X64).unwrap();
        let (Instruction::Auipc { rd, imm: hi }, Instruction::Ld { rd: ld_rd, rs1, imm: lo }) =
            (instrs[4], instrs[5])
        else {
            panic!("expected auipc; ld, got {:?}", &instrs[4..6]);
        };
        assert_eq!((rd, ld_rd, rs1), (XReg::A0, XReg::A0, XReg::A0));
        let pc = block + 4 * 4;
        let loaded = pc.wrapping_add(((hi << 12) as i64) as u64).wrapping_add(lo as i64 as u64);
        assert_eq!(loaded, golden_addr + 3 * GOLDEN_SLOT_SIZE);
        // The draft's XORs read x0, so the slot holds the destination itself.
        assert_eq!(site.golden_entry(), Some((3, 0)));
        assert!(matches!(instrs[6], Instruction::Xor { rs2: XReg::ZERO, .. }));

        // Once resolved, golden ^ a1 ^ a2 must give the destination (0).
        let mut xregs = [0u64; 32];
        xregs[XReg::A1.index() as usize] = 0x1234_5678_9abc_def0;
        xregs[XReg::A2.index() as usize] = 0x0fed_cba9_8765_4321;
        let target = test_target();
        site.resolve(&xregs, &target, &mut rand::rngs::StdRng::seed_from_u64(0)).unwrap();
        site.render(&mut instrs, Xlen::X64).unwrap();
        let (_, golden) = site.golden_entry().unwrap();
        assert_eq!(golden ^ xregs[XReg::A1.index() as usize] ^ xregs[XReg::A2.index() as usize], 0);
        assert!(matches!(instrs[6], Instruction::Xor { rs2: XReg::A1, .. }));
        assert!(matches!(instrs[5], Instruction::Ld { .. }), "patching must not touch the load");
    }

    #[test]
    fn written_register_is_decoded() {
        let add = Instruction::Add {
            rd: XReg::A0,
            rs1: XReg::A1,
            rs2: XReg::A2,
        };
        let store = Instruction::Sw {
            rs1: XReg::A0,
            rs2: XReg::A1,
            imm: 0,
        };
        assert_eq!(written_xreg(&add), Some(XReg::A0));
        assert_eq!(written_xreg(&store), None);
        assert_eq!(written_xreg(&Instruction::nop()), None);
    }
}
