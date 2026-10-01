use crate::entangle::{Site, SiteBuilder};
use crate::orchestrator::GlobalState;
use crate::memory::Section;
use crate::riscv::asmutil::{csr_rd_and_addr, with_csr};
use crate::riscv::asmutil::{load_imm, load_imm32};
use crate::riscv::{Instruction, InstructionClass, Opcode, XReg, Xlen};
use crate::target::Target;
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

    /// Generate the block's workload. With `sites`, memory base addresses are
    /// derived from workload registers and the block ends with entanglement
    /// sites checking the registers it wrote (see [`crate::entangle`]).
    pub fn run(
        &mut self,
        rng: &mut (impl Rng + ?Sized),
        target: &Target,
        state: &mut GlobalState,
        mut sites: Option<&mut SiteBuilder>,
    ) -> Result<()> {
        let candidates: Vec<_> = target
            .workload_opcodes()
            .filter(|opcode| {
                !matches!(
                    opcode.class(),
                    InstructionClass::Branch | InstructionClass::Jal | InstructionClass::Jalr
                ) && !matches!(
                    opcode,
                    Opcode::Ecall | Opcode::Ebreak | Opcode::CEbreak | Opcode::Sret | Opcode::Mret
                )
            })
            .collect();
        ensure!(
            self.budget == 0 || !candidates.is_empty(),
            "no enabled instructions remain for a straight-line workload"
        );
        let xlen = target.xlen;
        let data_sections: Vec<_> = state.memory.data_sections().collect();
        let csrs = target.workload_csrs();
        for _ in 0..self.budget {
            let opcode = candidates[rng.random_range(0..candidates.len())];
            let mut instr = opcode.random(rng, xlen)?;
            if csr_rd_and_addr(&instr).is_some() {
                instr = with_csr(instr, csrs[rng.random_range(0..csrs.len())]);
            }
            if let Some((base, addr)) = confine_memory_access(&instr, &data_sections, rng)? {
                match sites.as_deref_mut() {
                    Some(builder) if base != XReg::ZERO => {
                        let site = builder.address(&mut self.instrs, base, addr, rng);
                        self.sites.push(site);
                    }
                    _ => self.instrs.extend(load_imm(base, addr as i64, xlen)),
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

/// Point a memory access at a random aligned address inside one of `sections`
/// and return its address register with the base value it must hold. The
/// sampled immediate is kept, so the encoding constraints still hold.
fn confine_memory_access(
    instr: &Instruction,
    sections: &[&Section],
    rng: &mut (impl Rng + ?Sized),
) -> Result<Option<(XReg, u64)>> {
    let Some((base, imm, size)) = memory_operand(instr)? else {
        return Ok(None);
    };
    ensure!(!sections.is_empty(), "no writable section for memory accesses");
    let region = &sections[rng.random_range(0..sections.len())].region;
    let first = region.start.next_multiple_of(size);
    let last = region.end()?.saturating_sub(size) / size * size;
    ensure!(
        region.size >= size && first <= last,
        "memory region at {:#x} is too small for a {size}-byte access",
        region.start
    );
    let addr = rng.random_range(first / size..=last / size) * size;
    Ok(Some((base, addr.wrapping_sub(imm as u64))))
}

/// The address register, immediate offset and access width of a memory access.
fn memory_operand(instr: &Instruction) -> Result<Option<(XReg, i64, u64)>> {
    use Instruction::*;
    // AMO/LR/SC have no immediate: the address is rs1 itself.
    let class = instr.class();
    if matches!(class, InstructionClass::Amo | InstructionClass::Amo64) {
        let size = if class == InstructionClass::Amo64 { 8 } else { 4 };
        let rs1 = XReg::new(((instr.encode()?.bits() >> 15) & 31) as u8)?;
        return Ok(Some((rs1, 0, size)));
    }
    Ok(match *instr {
        Lb { rs1, imm, .. } | Lbu { rs1, imm, .. } | Sb { rs1, imm, .. } => {
            Some((rs1, imm.into(), 1))
        }
        Lh { rs1, imm, .. } | Lhu { rs1, imm, .. } | Sh { rs1, imm, .. } => {
            Some((rs1, imm.into(), 2))
        }
        Lw { rs1, imm, .. }
        | Lwu { rs1, imm, .. }
        | Sw { rs1, imm, .. }
        | Flw { rs1, imm, .. }
        | Fsw { rs1, imm, .. }
        | CLw { rs1, imm, .. }
        | CSw { rs1, imm, .. } => Some((rs1, imm.into(), 4)),
        Ld { rs1, imm, .. }
        | Sd { rs1, imm, .. }
        | Fld { rs1, imm, .. }
        | Fsd { rs1, imm, .. }
        | CLd { rs1, imm, .. }
        | CSd { rs1, imm, .. } => Some((rs1, imm.into(), 8)),
        CLwsp { imm, .. } | CSwsp { imm, .. } => Some((XReg::SP, imm.into(), 4)),
        CLdsp { imm, .. } | CSdsp { imm, .. } => Some((XReg::SP, imm.into(), 8)),
        _ => None,
    })
}
