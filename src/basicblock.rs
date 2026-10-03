use crate::entangle::{Site, SiteBuilder, written_xreg};
use crate::hart::HartState;
use crate::orchestrator::GlobalState;
use crate::riscv::asmutil::{csr_rd_and_addr, with_csr};
use crate::riscv::asmutil::load_imm32;
use crate::riscv::{Instruction, XReg, Xlen};
use crate::target::Target;
use crate::weights::{InstrWeights, is_control_flow, unweightable_reason};
use anyhow::{Result, ensure};
use rand::{Rng, RngExt};

#[derive(Default)]
pub struct BasicBlock {
    pub id: usize,
    pub is_smc: bool,
    pub budget: usize,
    pub instrs: Vec<Instruction>,
    /// Exported as a function symbol at the block's first instruction.
    pub label: Option<String>,
    /// Entanglement sites within `instrs`, in program order.
    pub sites: Vec<Site>,
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
        }
    }

    /// Generate the block's workload, drawing opcodes from `weights`. With `sites`, memory base addresses are
    /// derived from workload registers and the block ends with entanglement
    /// sites checking the registers it wrote (see [`crate::entangle`]).
    pub fn run(
        &mut self,
        rng: &mut (impl Rng + ?Sized),
        target: &Target,
        _state: &mut GlobalState,
        hart_state: &mut HartState,
        weights: &InstrWeights,
        mut sites: Option<&mut SiteBuilder>,
    ) -> Result<()> {
        // Branches and jalr are drawn like any opcode, but only exist as
        // entanglement sites.
        let candidates: Vec<_> = target
            .workload_opcodes()
            .filter(|opcode| unweightable_reason(*opcode).is_none())
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
        let csrs = target.workload_csrs();

        for _ in 0..self.budget {
            let opcode = sampler.as_ref().expect("built for a nonzero budget").sample(rng);
            if is_control_flow(opcode) {
                let builder = sites
                    .as_deref_mut()
                    .expect("control flow is only drawn when entangling");
                let site = builder.control_flow(&mut self.instrs, opcode, weights, rng);
                self.sites.extend(site);
                continue;
            }
            let mut instr = opcode.random(rng, xlen)?;
            if csr_rd_and_addr(&instr).is_some() {
                instr = with_csr(instr, csrs[rng.random_range(0..csrs.len())]);
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

            // If current instr reads an implementation-dependent CSR into rd,
            // overwrite rd with a random value so the test stays deterministic.
            if let Some((rd, csr)) = csr_rd_and_addr(&instr) {
                if rd != XReg::ZERO && csr.is_implementation_dependent() {
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
        Ok(())
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
