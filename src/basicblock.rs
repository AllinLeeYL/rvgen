use crate::context::GenContext;
use crate::riscv::asmutil::load_imm32;
use crate::riscv::fields::Csr;
use crate::riscv::{Instruction, InstructionClass, Opcode, XReg, Xlen};
use anyhow::{Ok, Result};
use rand::{Rng, RngExt};

#[derive(Default)]
pub struct BasicBlock {
    pub id: usize,
    pub budget: usize,
    pub instrs: Vec<Instruction>,
}

impl BasicBlock {
    pub fn new(id: usize, budget: usize) -> Self {
        Self {
            id: id,
            budget: budget,
            instrs: Vec::with_capacity(budget),
        }
    }

    pub fn run(&mut self, rng: &mut (impl Rng + ?Sized), ctx: &GenContext) -> Result<()> {
        let mut disabled_instrs = ctx.disabled_instrs.clone();
        // These classes include both standard and compressed branches and jumps.
        for class in [
            InstructionClass::Branch,
            InstructionClass::Jal,
            InstructionClass::Jalr,
        ] {
            disabled_instrs.extend(class.opcodes());
        }
        // Explicit traps and trap returns also transfer control out of the block.
        disabled_instrs.extend([
            Opcode::Ecall,
            Opcode::Ebreak,
            Opcode::CEbreak,
            Opcode::Sret,
            Opcode::Mret,
        ]);

        for _ in 0..self.budget {
            let opcode = choose_instr_with_rng(rng, &disabled_instrs);
            let instr = opcode.random(rng, Xlen::X64)?;
            self.instrs.push(instr);

            // If current instr reads an implementation-dependent CSR into rd,
            // overwrite rd with a random value so the test stays deterministic.
            if let Some((rd, csr)) = csr_rd_and_addr(&instr) {
                if rd != XReg::ZERO && csr.is_implementation_dependent() {
                    let fixup = load_imm32(rd, rng.random::<i32>(), Xlen::X64);
                    self.instrs.extend_from_slice(&fixup);
                }
            }
        }
        Ok(())
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::with_capacity(self.instrs.len() * 4);
        for instr in &self.instrs {
            instr.append_bytes(&mut bytes)?;
        }
        Ok(bytes)
    }
}

/// Uniformly select a named opcode for the current RV64 target.
/// Operand construction and execution-state constraints are left to the caller.
pub fn choose_instr_with_rng(rng: &mut (impl Rng + ?Sized), disabled_instrs: &[Opcode]) -> Opcode {
    loop {
        let opcode = Opcode::ALL[rng.random_range(0..Opcode::ALL.len())];
        if !disabled_instrs.contains(&opcode) && opcode.supports_xlen(Xlen::X64) {
            return opcode;
        }
    }
}

/// Extract the destination register and CSR address from a CSR instruction.
/// Returns `None` for non-CSR instructions.
fn csr_rd_and_addr(instr: &Instruction) -> Option<(XReg, Csr)> {
    match *instr {
        Instruction::Csrrw { rd, csr, .. }
        | Instruction::Csrrs { rd, csr, .. }
        | Instruction::Csrrc { rd, csr, .. }
        | Instruction::Csrrwi { rd, csr, .. }
        | Instruction::Csrrsi { rd, csr, .. }
        | Instruction::Csrrci { rd, csr, .. } => Some((rd, csr)),
        _ => None,
    }
}
