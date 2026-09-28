//! Shared target, memory plan, generation policy, and machine-mode ELF runtime.
//!
//! Construct one Runtime before generating a workload. Both generation and
//! startup consult its Target; memory bounds and ELF sections share MemoryPlan.
//! Code emission and address resolution live in private implementation modules.
mod code;
pub mod memory;
pub mod target;

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result, ensure};

use crate::riscv::{Extension, Opcode, XReg, Xlen};
use crate::utils::build_elf_with_symbols;
use code::RuntimeCode;
use memory::MemoryPlan;
pub use target::Target;

/// Reserved by generation: gp anchors scratch memory; tp points to optional SMC.
pub const SCRATCH_BASE_REGISTER: XReg = XReg::GP;
pub const SMC_BASE_REGISTER: XReg = XReg::TP;

pub struct Runtime {
    target: Target,
    memory: MemoryPlan,
    disabled_opcodes: HashSet<Opcode>,
}

impl Runtime {
    pub fn new(
        target: Target,
        memory: MemoryPlan,
        disabled: impl IntoIterator<Item = Opcode>,
    ) -> Result<Self> {
        ensure!(
            target.has(Extension::Zicsr),
            "machine-mode runtime requires Zicsr; use --isa i,zicsr (F/D imply Zicsr)"
        );
        memory.validate_xlen(target.xlen())?;
        for name in [".tohost", ".fromhost"] {
            let section = memory
                .request(name)
                .with_context(|| format!("runtime requires {name}"))?;
            ensure!(
                section.size >= 8 && section.alignment >= 8 && section.permissions.write,
                "{name} requires at least 8 writable bytes aligned to 8"
            );
        }
        let scratch = memory
            .request(".scratch")
            .context("runtime requires .scratch")?;
        ensure!(
            scratch.size >= 16 && scratch.alignment >= 8 && scratch.permissions.write,
            ".scratch requires at least 16 writable bytes aligned to 8"
        );
        if let Some(smc) = memory.request(".smc") {
            ensure!(
                smc.alignment >= 4 && smc.permissions.write && smc.permissions.execute,
                ".smc must be writable, executable, and four-byte aligned"
            );
        }
        Ok(Self {
            target,
            memory,
            disabled_opcodes: disabled.into_iter().collect(),
        })
    }

    pub fn target(&self) -> &Target {
        &self.target
    }
    pub fn memory(&self) -> &MemoryPlan {
        &self.memory
    }

    /// Disabling opcodes is a workload policy, not removal of runtime capabilities.
    pub fn workload_opcodes(&self) -> impl Iterator<Item = Opcode> + '_ {
        Opcode::ALL.iter().copied().filter(|opcode| {
            self.target.supports(*opcode) && !self.disabled_opcodes.contains(opcode)
        })
    }

    pub fn scratch_window(&self) -> ScratchWindow {
        let size = self
            .memory
            .request(".scratch")
            .expect("validated scratch section")
            .size as u64;
        ScratchWindow {
            size,
            base_offset: (size.min(4096) / 2) & !7,
        }
    }

    pub fn executable(&self, body: &[u8]) -> Result<Vec<u8>> {
        ensure!(
            body.len() % self.target.instruction_alignment() == 0,
            "workload length must be aligned to {} bytes for this ISA",
            self.target.instruction_alignment()
        );
        let mut code = RuntimeCode::assemble(body, self)?;
        let layout = self.memory.resolve(code.bytes.len())?;
        code.resolve_addresses(&layout)?;
        let symbols = code.symbols(&layout)?;
        let sections = layout.sections(code.bytes)?;
        build_elf_with_symbols(
            &sections,
            &symbols,
            self.target.xlen() == Xlen::X64,
            self.memory.entry(),
        )
    }

    pub fn write_executable(&self, body: &[u8], path: impl AsRef<Path>) -> Result<()> {
        let bytes = self.executable(body)?;
        let path = path.as_ref();
        std::fs::write(path, bytes)
            .with_context(|| format!("failed to write ELF to {}", path.display()))
    }
}

/// Offsets relative to gp, derived from the same .scratch request used by layout.
#[derive(Debug, Clone, Copy)]
pub struct ScratchWindow {
    size: u64,
    base_offset: u64,
}

impl ScratchWindow {
    pub fn base_offset(self) -> u64 {
        self.base_offset
    }

    pub fn offset_bounds(
        self,
        access_size: i32,
        immediate_min: i32,
        immediate_max: i32,
    ) -> Result<(i32, i32)> {
        ensure!(
            access_size > 0 && (access_size as u32).is_power_of_two(),
            "invalid memory access size"
        );
        let low = (-(self.base_offset as i64)).max(immediate_min as i64);
        let high = (self.size as i128 - self.base_offset as i128 - access_size as i128)
            .min(immediate_max as i128);
        ensure!(high >= low as i128, "memory access does not fit .scratch");
        let size = access_size as i64;
        let first = -(-low).div_euclid(size) * size;
        let last = (high as i64).div_euclid(size) * size;
        ensure!(first <= last, "no aligned memory access fits .scratch");
        Ok((first as i32, last as i32))
    }
}

#[cfg(test)]
mod tests;
