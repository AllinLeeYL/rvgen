use crate::context::GenContext;
use crate::riscv::{Instruction, Opcode, Xlen};
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

    pub fn run(&mut self, rng: &mut (impl Rng + ?Sized), cxt: &GenContext) -> Result<()> {
        for _ in 0..self.budget {
            let opcode = choose_instr_with_rng(rng, &cxt.disabled_instrs);
            let instr = opcode.random(rng, Xlen::X64)?;
            self.instrs.push(instr);
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
