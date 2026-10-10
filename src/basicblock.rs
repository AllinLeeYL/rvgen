use crate::csrs::{CsrSampler, Read};
use crate::entangle::{Dest, Kind, PoolRef, Site, SiteBuilder, written_xreg};
use crate::hart::HartState;
use crate::orchestrator::GlobalState;
use crate::riscv::asmutil::{csr_rd_and_addr, split_imm32};
use crate::riscv::asmutil::load_imm32;
use crate::riscv::{Instruction, InstructionClass, XReg, Xlen};
use crate::target::Target;
use crate::weights::{InstrWeights, is_control_flow, unweightable_reason};
use anyhow::{Result, ensure};
use rand::{Rng, RngExt};

#[derive(Default, Clone)]
pub struct BasicBlock {
    pub id: usize,
    pub is_smc: bool,
    pub budget: usize,
    pub instrs: Vec<Instruction>,
    /// Exported as a function symbol at the block's first instruction.
    pub label: Option<String>,
    /// Entanglement sites within `instrs`, in program order.
    pub sites: Vec<Site>,
    /// References to `.golden` within `instrs` outside of sites.
    pub pool_refs: Vec<PoolRef>,
    /// Address of the first instruction, set by `Hart::place`.
    pub addr: u64,
    /// Index of the block this one is placed right after, e.g. the caller a
    /// `ret` returns into; `None` places it anywhere.
    pub after: Option<usize>,
    /// The key a trap vector names this block by, when a trap lands here
    /// (see [`crate::privilege`]). Such a block is 4-byte aligned.
    pub landing: Option<usize>,
    /// Alignment of the block's address in bytes beyond the instructions',
    /// if nonzero.
    pub align: u64,
    /// Store sites writing SMC blocks' code, in `instrs`.
    pub smc_stores: Vec<SmcStore>,
    /// Index of the earlier block whose address this SMC block reuses.
    pub alias: Option<usize>,
}

impl BasicBlock {
    pub fn new(id: usize, is_smc: bool, budget: usize) -> Self {
        Self {
            id: id,
            is_smc: is_smc,
            budget: budget,
            instrs: Vec::with_capacity(budget),
            label: None,
            sites: Vec::new(),
            pool_refs: Vec::new(),
            addr: 0,
            after: None,
            landing: None,
            align: 0,
            smc_stores: Vec::new(),
            alias: None,
        }
    }

    /// End the block with a jump to the next one: `jal rd, next`, or `c.j next`
    /// if `compressed`.
    pub fn jump(&mut self, rd: XReg, compressed: bool) {
        self.sites.push(Site {
            at: self.instrs.len(),
            kind: Kind::Jump { rd, compressed, offset: 0 },
            fail_at: None,
        });
        self.instrs.push(if compressed { Instruction::cnop() } else { Instruction::nop() });
    }

    /// End a callee's block with a return to the next block, which is placed
    /// right after the caller: `ra` is first pointed back at it, as nested
    /// calls and site code may have overwritten it, then `ret`, or `c.jr ra`
    /// if `compressed`.
    pub fn ret(&mut self, compressed: bool) {
        self.sites.push(Site {
            at: self.instrs.len(),
            kind: Kind::Link { rd: XReg::RA, offset: 0, dest: Dest::Next },
            fail_at: None,
        });
        self.instrs.extend([Instruction::nop(), Instruction::nop()]);
        self.instrs.push(if compressed {
            Instruction::CJr { rs1: XReg::RA }
        } else {
            Instruction::Jalr { rd: XReg::ZERO, rs1: XReg::RA, imm: 0 }
        });
    }

    /// Generate the block's workload, drawing opcodes from `weights`. With `sites`, memory base addresses are
    /// derived from workload registers and the block ends with entanglement
    /// sites checking the registers it wrote (see [`crate::entangle`]).
    /// Returns the unused budget if a control-flow site ended the block, which
    /// then needs no other way out.
    pub fn run(
        &mut self,
        rng: &mut (impl Rng + ?Sized),
        target: &Target,
        _state: &mut GlobalState,
        hart_state: &mut HartState,
        weights: &InstrWeights,
        mut sites: Option<&mut SiteBuilder>,
    ) -> Result<Option<usize>> {
        // Branches and jalr are drawn like any opcode, but only exist as
        // entanglement sites. Only what the current privilege state allows
        // is drawn, CSR accesses only if some CSR is accessible.
        let csrs = CsrSampler::new(target, &hart_state.privilege);
        let candidates: Vec<_> = target
            .workload_opcodes()
            .filter(|opcode| unweightable_reason(*opcode).is_none())
            .filter(|opcode| hart_state.privilege.allows(*opcode))
            .filter(|opcode| csrs.is_some() || opcode.class() != InstructionClass::Csr)
            .filter(|opcode| {
                !is_control_flow(*opcode)
                    || sites.as_deref().is_some_and(|builder| builder.can_generate(*opcode))
            })
            .collect();
        ensure!(
            self.budget == 0 || !candidates.is_empty(),
            "no enabled instructions remain for a straight-line workload"
        );
        let sampler = (self.budget > 0)
            .then(|| weights.sampler(&candidates))
            .transpose()?;
        let xlen = target.xlen;

        for drawn in 0..self.budget {
            let opcode = sampler.as_ref().expect("built for a nonzero budget").sample(rng);
            if is_control_flow(opcode) {
                let builder = sites
                    .as_deref_mut()
                    .expect("control flow is only drawn when entangling");
                let site = builder.control_flow(&mut self.instrs, opcode, weights, rng);
                let ends = site.ends_block();
                self.sites.push(site);
                if ends {
                    // Unchecked registers carry over to the next block's guards.
                    return Ok(Some(self.budget - drawn - 1));
                }
                continue;
            }
            let mut instr = opcode.random(rng, xlen)?;
            let mut csr_read = None;
            if csr_rd_and_addr(&instr).is_some() {
                let csr = csrs.as_ref().expect("CSR accesses are drawn with CSRs").sample(rng);
                instr = csr.access(instr);
                csr_read = Some(csr.read);
            }
            // If current instr is mem op, point it into a data section
            // through one of the program's base registers.
            let dep = sites.as_deref().and_then(SiteBuilder::freshest);
            if let Some(placement) = hart_state.mem_bases.place(instr, dep, rng)? {
                instr = placement.instr;
                for prefix in placement.prefix {
                    self.instrs.push(prefix);
                    if let Some(builder) = sites.as_deref_mut() {
                        // A data-dependent address carries workload data, so
                        // guards check it; a fixed one does not.
                        if placement.data_dependent {
                            builder.observe(&prefix);
                        } else if let Some(rd) = written_xreg(&prefix) {
                            builder.clobber(rd);
                        }
                    }
                }
            }
            self.instrs.push(instr);
            if let Some(builder) = sites.as_deref_mut() {
                builder.observe(&instr);
            }

            // If current instr reads a CSR whose value may differ from Spike's
            // into rd, overwrite rd with a random value so the test stays
            // deterministic.
            if let Some((rd, _)) = csr_rd_and_addr(&instr) {
                if rd != XReg::ZERO && csr_read == Some(Read::Clobbered) {
                    let fixup = load_imm32(rd, rng.random::<i32>(), xlen);
                    self.instrs.extend_from_slice(&fixup);
                    if let Some(builder) = sites.as_deref_mut() {
                        builder.clobber(rd);
                    }
                }
            }
            if let Some(builder) = sites.as_deref_mut() {
                let guard = builder.after_instr(&mut self.instrs, rng);
                self.sites.extend(guard);
            }
        }
        if let Some(builder) = sites {
            let end = builder.block_end(&mut self.instrs, rng);
            self.sites.extend(end);
        }
        Ok(None)
    }

    /// Insert `instrs` at the start of the block, shifting every recorded index.
    pub fn prepend(&mut self, instrs: &[Instruction]) {
        let n = instrs.len();
        self.instrs.splice(0..0, instrs.iter().copied());
        for site in &mut self.sites {
            site.at += n;
            if let Some(fail_at) = &mut site.fail_at {
                *fail_at += n;
            }
        }
        for pool_ref in &mut self.pool_refs {
            pool_ref.at += n;
        }
        for store in &mut self.smc_stores {
            store.at += n;
        }
    }

    /// Rewrite every site's instructions for what is currently known.
    pub fn render_sites(&mut self, xlen: Xlen) -> Result<()> {
        for site in &self.sites {
            site.render(&mut self.instrs, xlen)?;
        }
        Ok(())
    }

    pub fn encode(&self, target: &Target) -> Result<Vec<u8>> {
        let mut bytes = Vec::with_capacity(self.instrs.len() * 4);
        for instr in &self.instrs {
            target.emit(*instr, &mut bytes)?;
        }
        Ok(bytes)
    }
}


/// Writes block `smc`'s code through `rptr`, one store of `widths[k]` bytes
/// at a time, from instruction `at` of its host block on. `rval` ends at 0 so
/// the draft and the final program leave the same registers.
#[derive(Debug, Clone)]
pub struct SmcStore {
    pub at: usize,
    pub smc: usize,
    pub rptr: XReg,
    pub rval: XReg,
    pub widths: Vec<usize>,
}

impl SmcStore {
    pub fn len(&self) -> usize {
        2 + 3 * self.widths.len() + 1
    }

    /// Write the stores of `code`, the SMC block at `smc_addr`; `pc` is the
    /// address of `instrs[self.at]`.
    pub fn render(&self, instrs: &mut [Instruction], pc: u64, smc_addr: u64, code: &[u8], xlen: Xlen) {
        use Instruction::*;
        let (hi, lo) = split_imm32(smc_addr.wrapping_sub(pc) as u32);
        let (rptr, rval) = (self.rptr, self.rval);
        let mut out = vec![Auipc { rd: rptr, imm: hi }, Addi { rd: rptr, rs1: rptr, imm: lo }];
        let mut off = 0;
        for &width in &self.widths {
            let value = code[off..off + width].iter().rev().fold(0u32, |acc, byte| acc << 8 | *byte as u32);
            out.extend(load_imm32(rval, value as i32, xlen));
            let imm = off as i32;
            out.push(match width {
                1 => Sb { rs1: rptr, rs2: rval, imm },
                2 => Sh { rs1: rptr, rs2: rval, imm },
                _ => Sw { rs1: rptr, rs2: rval, imm },
            });
            off += width;
        }
        out.push(Addi { rd: rval, rs1: XReg::ZERO, imm: 0 });
        debug_assert_eq!(out.len(), self.len());
        instrs[self.at..self.at + out.len()].copy_from_slice(&out);
    }
}

/// Store widths covering `size` bytes at an address aligned to `unit`, each
/// naturally aligned, mixed at random.
pub fn store_widths(size: usize, unit: usize, rng: &mut (impl Rng + ?Sized)) -> Vec<usize> {
    let mut widths = Vec::new();
    let mut off = 0;
    while off < size {
        let fits: Vec<_> = [1, 2, 4]
            .into_iter()
            .filter(|&width| width <= unit && off % width == 0 && off + width <= size)
            .collect();
        let width = fits[rng.random_range(0..fits.len())];
        widths.push(width);
        off += width;
    }
    widths
}
