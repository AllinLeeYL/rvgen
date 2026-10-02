//! Data/control-flow entanglement, after Cascade.
//!
//! Sites make the program's path depend on register values computed by the
//! workload, so a core that miscomputes one diverges instead of carrying on
//! silently:
//!
//! - A *value* site builds a register as `r = K; r ^= dep_1; ...; r ^= dep_k`,
//!   where the `dep_i` hold workload data and `K` is patched to
//!   `dest ^ dep_1 ^ ... ^ dep_k` once Spike tells us their values. It serves
//!   as an indirect jump target, or (with `dest = 0`) the accumulator of a
//!   guard.
//! - A *branch* site compares two workload registers. Its opcode is picked
//!   once Spike tells us their values, so the planned direction is the right
//!   one; the wrong direction lands on a jump to `_fail`.
//! - A *guard* tests a value site's accumulator against zero with `beq`/`bne`.
//!   Unlike the others, which only notice faults that move a jump target or a
//!   comparison far enough, it notices any wrong value among its `dep_i`.
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
use rand::{Rng, RngExt};

use crate::riscv::asmutil::{load_imm_fixed, to_unsigned, twos_complement};
use crate::riscv::{EncodedInstruction, Instruction, Opcode, XReg, Xlen};
use crate::target::Target;

/// Branch opcodes as complementary pairs: for any operands, exactly one of each
/// pair is taken, so an enabled pair can always realize either direction.
const BRANCH_PAIRS: [(Opcode, Opcode); 3] = [
    (Opcode::Beq, Opcode::Bne),
    (Opcode::Blt, Opcode::Bge),
    (Opcode::Bltu, Opcode::Bgeu),
];

/// Most workload registers a jump target is entangled with.
const MAX_PEEK: usize = 4;

/// Probability that a block ends with a branch or indirect jump before its
/// guard.
const CONTROL_FLOW_PROBA: f64 = 0.5;

/// Probability that such a control-flow site is an indirect jump.
const JALR_PROBA: f64 = 0.3;


/// Where a value site's register must end up pointing.
#[derive(Debug, Clone, Copy)]
pub enum Dest {
    /// An absolute value: 0 for a guard.
    Data(u64),
    /// The instruction at this index of the same block (possibly one past the
    /// end, i.e. the next block). Resolved to an address by [`Site::link`].
    Code(usize),
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
    },
    Branch {
        rs1: XReg,
        rs2: XReg,
        taken: bool,
        /// Chosen once Spike tells us the operands' values.
        opcode: Option<Opcode>,
    },
    /// `beq reg, x0` when `taken`, `bne reg, x0` otherwise; `reg` is 0 in a
    /// correct execution.
    Guard { reg: XReg, taken: bool },
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
    /// Number of instructions in the producer sequence of a value site.
    fn producer_len(xlen: Xlen) -> usize {
        load_imm_fixed(XReg::ZERO, 0, xlen).len()
    }

    /// Index of the instruction at which Spike must report the registers the
    /// site depends on, if it depends on any: the first XOR of a value site,
    /// the branch itself for a branch site.
    pub fn probe_index(&self, xlen: Xlen) -> Option<usize> {
        match self.kind {
            Kind::Value { .. } => Some(self.at + Self::producer_len(xlen)),
            Kind::Branch { .. } => Some(self.at),
            Kind::Guard { .. } => None,
        }
    }

    /// Fix code addresses: `block_addr` is the absolute address of the
    /// block's first instruction and `fail_addr` that of `_fail`.
    pub fn link(&mut self, instrs: &mut [Instruction], block_addr: u64, fail_addr: u64) -> Result<()> {
        let addr_of = |instrs: &[Instruction], index: usize| {
            block_addr + code_size(&instrs[..index]) as u64
        };
        if let Some(fail_at) = self.fail_at {
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
        if let Kind::Value { dest, value, .. } = &mut self.kind {
            *value = Some(match *dest {
                Dest::Data(value) => value,
                Dest::Code(index) => addr_of(instrs, index),
            });
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
                opcode,
            } => {
                let a = xregs[rs1.index() as usize];
                let b = xregs[rs2.index() as usize];
                let candidates: Vec<_> = enabled_branch_opcodes(target)
                    .filter(|op| branch_taken(*op, a, b, xlen) == *taken)
                    .collect();
                ensure!(!candidates.is_empty(), "no enabled branch realizes the planned direction");
                *opcode = Some(candidates[rng.random_range(0..candidates.len())]);
            }
            Kind::Guard { .. } => {}
        }
        Ok(())
    }

    /// Write the site's instructions for what is currently known: the
    /// placeholder before [`Site::resolve`], the entangled form after.
    pub fn render(&self, instrs: &mut [Instruction], xlen: Xlen) -> Result<()> {
        // Branches skip the next instruction when taken.
        match self.kind {
            Kind::Value {
                rprod,
                ref rdeps,
                value,
                rdep_val,
                ..
            } => {
                let value = value.ok_or_else(|| anyhow!("site rendered before linking"))?;
                // Draft: rprod = value ^ 0 ^ ... ^ 0.
                // Final: rprod = (value ^ rdeps) ^ rdep_1 ^ ... ^ rdep_k.
                let constant = value ^ rdep_val.unwrap_or(0);
                let producer = load_imm_fixed(rprod, constant as i64, xlen);
                let n = producer.len();
                instrs[self.at..self.at + n].copy_from_slice(&producer);
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
            } => {
                instrs[self.at] = match opcode {
                    Some(op) => branch(op, rs1, rs2, 8)?,
                    None if taken => Instruction::Jal {
                        rd: XReg::ZERO,
                        imm: 8,
                    },
                    None => Instruction::nop(),
                };
            }
            Kind::Guard { reg, taken } => {
                let op = if taken { Opcode::Beq } else { Opcode::Bne };
                instrs[self.at] = branch(op, reg, XReg::ZERO, 8)?;
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
}

impl<'a> SiteBuilder<'a> {
    pub fn new(target: &'a Target, guard_threshold: usize, reserved: Vec<XReg>) -> Self {
        Self {
            target,
            fresh: Vec::new(),
            guard_threshold,
            reserved,
        }
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

    /// Append the sites ending a block: maybe a branch or indirect jump, then
    /// a guard over every fresh register.
    pub fn block_end(&mut self, instrs: &mut Vec<Instruction>, rng: &mut (impl Rng + ?Sized)) -> Vec<Site> {
        let mut sites = Vec::new();
        if rng.random_bool(CONTROL_FLOW_PROBA) {
            sites.extend(self.control_flow(instrs, rng));
        }
        if self.can_guard() && !self.fresh.is_empty() {
            sites.extend(self.guard(instrs, rng));
        }
        sites
    }

    /// A conditional branch or an indirect jump to the next instruction, if
    /// the enabled opcodes allow one.
    fn control_flow(&mut self, instrs: &mut Vec<Instruction>, rng: &mut (impl Rng + ?Sized)) -> Vec<Site> {
        let can_branch = BRANCH_PAIRS
            .iter()
            .any(|(a, b)| self.enabled(*a) && self.enabled(*b));
        let can_jalr = self.enabled(Opcode::Jalr);
        if can_jalr && (!can_branch || rng.random_bool(JALR_PROBA)) {
            let rprod = self.stale(rng);
            let rd = self.stale(rng);
            let rdeps = self.peek(rng, MAX_PEEK, rprod);
            self.clobber(rprod);
            self.clobber(rd);
            // producer; xor...; jalr rd, 0(rprod); jal x0, _fail; <dest>
            let at = instrs.len();
            let dest = at + Site::producer_len(self.target.xlen) + rdeps.len() + 2;
            let mut site = self.value_site(instrs, rprod, rdeps, Dest::Code(dest));
            instrs.push(Instruction::Jalr { rd, rs1: rprod, imm: 0 });
            site.fail_at = Some(instrs.len());
            instrs.push(Instruction::nop());
            return vec![site];
        }
        if !can_branch {
            return Vec::new();
        }
        let operands = self.peek(rng, 2, XReg::ZERO);
        let rs1 = operands[0];
        let rs2 = match operands.get(1) {
            Some(reg) => *reg,
            None => self.random_reg_except(rng, rs1),
        };
        let taken = rng.random_bool(0.5);
        vec![branch_site(instrs, Kind::Branch { rs1, rs2, taken, opcode: None }, taken)]
    }

    /// `acc = K ^ fresh_1 ^ ... ^ fresh_k`, which is 0 in a correct execution,
    /// then a branch to `_fail` unless it is. Consumes every fresh register.
    fn guard(&mut self, instrs: &mut Vec<Instruction>, rng: &mut (impl Rng + ?Sized)) -> Vec<Site> {
        let acc = self.stale(rng);
        let rdeps: Vec<_> = self.fresh.drain(..).filter(|reg| *reg != acc).collect();
        let value = self.value_site(instrs, acc, rdeps, Dest::Data(0));
        let taken = rng.random_bool(0.5);
        let guard = branch_site(instrs, Kind::Guard { reg: acc, taken }, taken);
        vec![value, guard]
    }

    fn value_site(&self, instrs: &mut Vec<Instruction>, rprod: XReg, rdeps: Vec<XReg>, dest: Dest) -> Site {
        let at = instrs.len();
        // Placeholders; the site renders its real instructions once linked.
        let n = Site::producer_len(self.target.xlen) + rdeps.len();
        instrs.extend(std::iter::repeat_n(Instruction::nop(), n));
        Site {
            at,
            kind: Kind::Value {
                rprod,
                rdeps,
                dest,
                value: None,
                rdep_val: None,
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
        let stale: Vec<_> = (1..32)
            .map(|i| XReg::new(i).expect("valid register"))
            .filter(|reg| !self.fresh.contains(reg) && !self.reserved.contains(reg))
            .collect();
        if stale.is_empty() {
            self.random_reg_except(rng, XReg::ZERO)
        } else {
            stale[rng.random_range(0..stale.len())]
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

/// Append a branch site, which skips the next instruction when taken:
///   taken:     b<cc> +8; jal _fail
///   not taken: b<cc> +8; jal +8; jal _fail
fn branch_site(instrs: &mut Vec<Instruction>, kind: Kind, taken: bool) -> Site {
    let at = instrs.len();
    instrs.push(Instruction::nop());
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
        kind,
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
