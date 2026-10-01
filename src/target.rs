// The target defines the generation target.
// Target remain fixed during generation.
use std::collections::HashSet;

use anyhow::{Result, ensure};

use crate::{
    memory::MemoryRegion,
    riscv::{Csr, Extension, Instruction, Opcode, SAFE_CSRS, Xlen},
};

#[derive(Debug, Clone)]
pub struct Target {
    pub xlen: Xlen,
    pub extensions: HashSet<Extension>,
    pub disabled_opcodes: HashSet<Opcode>,
    pub num_cores: usize,
    pub num_instrs: usize,
    pub physical_memory: MemoryRegion,
}

impl Target {
    /// Resolve architectural dependencies once: D implies F.
    pub fn new(
        xlen: Xlen,
        extensions: impl IntoIterator<Item = Extension>,
        disabled_opcodes: HashSet<Opcode>,
        num_cores: usize,
        num_instrs: usize,
        physical_memory: MemoryRegion,
    ) -> Result<Self> {
        let mut extensions: HashSet<_> = extensions.into_iter().collect();
        ensure!(
            extensions.contains(&Extension::I),
            "the target requires the I base ISA"
        );
        ensure!(
            !extensions.contains(&Extension::Raw),
            "raw is an encoding escape hatch, not an ISA extension"
        );
        ensure!(
            extensions.contains(&Extension::Zicsr),
            "the target requires Zicsr to install the trap handler"
        );
        if extensions.contains(&Extension::D) {
            extensions.insert(Extension::F);
        }
        Ok(Self {
            xlen,
            extensions,
            disabled_opcodes,
            num_cores,
            num_instrs,
            physical_memory,
        })
    }

    pub fn has(&self, extension: Extension) -> bool {
        self.extensions.contains(&extension)
    }

    pub fn supports(&self, opcode: Opcode) -> bool {
        opcode.supports_xlen(self.xlen) && self.has(opcode.extension())
    }

    pub fn workload_opcodes(&self) -> impl Iterator<Item = Opcode> + '_ {
        Opcode::ALL
            .iter()
            .copied()
            .filter(|opcode| self.supports(*opcode) && !self.disabled_opcodes.contains(opcode))
    }

    /// CSRs the workload may access: the always-safe set plus those whose
    /// extension is enabled, so no CSR access traps as illegal.
    pub fn workload_csrs(&self) -> Vec<Csr> {
        let mut csrs = SAFE_CSRS.to_vec();
        if self.has(Extension::F) {
            csrs.push(Csr::FFLAGS);
        }
        csrs
    }

    /// ISA string for Spike, e.g. `rv64imafd_zicsr_zifencei`.
    pub fn spike_isa(&self) -> String {
        let mut isa = format!("rv{}i", self.xlen.bits());
        for (extension, letter) in [
            (Extension::M, 'm'),
            (Extension::A, 'a'),
            (Extension::F, 'f'),
            (Extension::D, 'd'),
            (Extension::C, 'c'),
        ] {
            if self.has(extension) {
                isa.push(letter);
            }
        }
        for (extension, name) in [
            (Extension::Zicsr, "zicsr"),
            (Extension::Zifencei, "zifencei"),
            (Extension::Svinval, "svinval"),
        ] {
            if self.has(extension) {
                isa.push('_');
                isa.push_str(name);
            }
        }
        isa
    }

    pub fn instruction_alignment(&self) -> usize {
        if self.has(Extension::C) { 2 } else { 4 }
    }

    /// Validate both ISA availability and operands before emitting any code.
    pub fn emit(&self, instruction: Instruction, bytes: &mut Vec<u8>) -> Result<()> {
        ensure!(
            self.supports(instruction.opcode()),
            "{} is not enabled for this target",
            instruction.opcode()
        );
        instruction.encode_for(self.xlen)?.append_bytes(bytes);
        Ok(())
    }
}
