//! Validated ISA capabilities shared by generation and runtime emission.
use std::collections::HashSet;

use anyhow::{Result, ensure};

use crate::riscv::{Extension, Instruction, Opcode, Xlen};

#[derive(Debug, Clone)]
pub struct Target {
    xlen: Xlen,
    extensions: HashSet<Extension>,
}

impl Target {
    /// Resolve architectural dependencies once: D implies F, F implies Zicsr.
    pub fn new(xlen: Xlen, extensions: impl IntoIterator<Item = Extension>) -> Result<Self> {
        let mut extensions: HashSet<_> = extensions.into_iter().collect();
        ensure!(
            extensions.contains(&Extension::I),
            "the target requires the I base ISA"
        );
        ensure!(
            !extensions.contains(&Extension::Raw),
            "raw is an encoding escape hatch, not an ISA extension"
        );
        if extensions.contains(&Extension::D) {
            extensions.insert(Extension::F);
        }
        if extensions.contains(&Extension::F) {
            extensions.insert(Extension::Zicsr);
        }
        Ok(Self { xlen, extensions })
    }

    pub fn xlen(&self) -> Xlen {
        self.xlen
    }

    pub fn has(&self, extension: Extension) -> bool {
        self.extensions.contains(&extension)
    }

    pub fn supports(&self, opcode: Opcode) -> bool {
        opcode.supports_xlen(self.xlen) && self.has(opcode.extension())
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
