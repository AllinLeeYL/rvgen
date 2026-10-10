// The target defines the generation target.
// Target remain fixed during generation.
use std::collections::HashSet;

use anyhow::{Result, ensure};

use crate::{
    memory::MemoryRegion,
    riscv::{Csr, Extension, Instruction, Opcode, PrivilegeLevel, SAFE_CSRS, Xlen},
};

#[derive(Debug, Clone)]
pub struct Target {
    pub xlen: Xlen,
    pub extensions: HashSet<Extension>,
    pub privileges: HashSet<PrivilegeLevel>,
    pub disabled_opcodes: HashSet<Opcode>,
    pub num_cores: usize,
    pub num_instrs: usize,
    pub physical_memory: MemoryRegion,
    
    /// Data section size; `None` draws one per program.
    pub scratch_size: Option<u64>,
    
    /// self-modifying-code section size
    pub smc_size: u64,
    
    /// PMP entries the target implements (pmpaddr0 on).
    pub pmp_regions: usize,
    
    /// `medeleg` bits the device and Spike both delegate; traps are only
    /// routed to S through these (see [`crate::privilege`]). 0 by default.
    pub medeleg_mask: u64,
    
    /// Whether misaligned loads and stores trap, so they can be raised on
    /// purpose. False by default.
    pub misaligned_traps: bool,

    /// Probability that a workload block is self-modifying (see
    /// [`crate::hart::Hart::plan_smc`]). 0 by default.
    pub smc_proba: f64,
}

impl Target {
    /// Resolve architectural dependencies once: D implies F.
    pub fn new(
        xlen: Xlen,
        extensions: impl IntoIterator<Item = Extension>,
        privileges: impl IntoIterator<Item = PrivilegeLevel>,
        disabled_opcodes: HashSet<Opcode>,
        num_cores: usize,
        num_instrs: usize,
        physical_memory: MemoryRegion,
        scratch_size: Option<u64>,
        smc_size: u64,
        pmp_regions: usize,
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
        let privileges: HashSet<_> = privileges.into_iter().collect();
        ensure!(
            privileges.contains(&PrivilegeLevel::Machine),
            "the target requires M-mode"
        );
        ensure!(
            !privileges.contains(&PrivilegeLevel::Supervisor)
                || privileges.contains(&PrivilegeLevel::User),
            "S-mode requires U-mode"
        );
        crate::csrs::validate_pmp_regions(pmp_regions)?;
        Ok(Self {
            xlen,
            extensions,
            privileges,
            disabled_opcodes,
            num_cores,
            num_instrs,
            physical_memory,
            scratch_size,
            smc_size,
            pmp_regions,
            medeleg_mask: 0,
            misaligned_traps: false,
            smc_proba: 0.0,
        })
    }

    pub fn has(&self, extension: Extension) -> bool {
        self.extensions.contains(&extension)
    }

    pub fn has_privilege(&self, privilege: PrivilegeLevel) -> bool {
        self.privileges.contains(&privilege)
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

    /// Plain XLEN-wide read/write scratch CSRs present on this target:
    /// mscratch always, sscratch only with S-mode.
    pub fn scratch_csrs(&self) -> Vec<Csr> {
        let mut csrs = SAFE_CSRS.to_vec();
        if self.has_privilege(PrivilegeLevel::Supervisor) {
            csrs.push(Csr::SSCRATCH);
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

    /// Privilege modes for Spike's `--priv`: `m`, `mu`, or `msu`.
    pub fn spike_priv(&self) -> String {
        [
            (PrivilegeLevel::Machine, 'm'),
            (PrivilegeLevel::Supervisor, 's'),
            (PrivilegeLevel::User, 'u'),
        ]
        .into_iter()
        .filter(|(privilege, _)| self.has_privilege(*privilege))
        .map(|(_, letter)| letter)
        .collect()
    }

    pub fn instruction_alignment(&self) -> usize {
        if self.has(Extension::C) { 2 } else { 4 }
    }

    /// Validate both ISA availability and operands before emitting any code.
    pub fn emit(&self, instruction: Instruction, bytes: &mut Vec<u8>) -> Result<()> {
        // Raw encodings fill the code area's gaps with junk; generator code
        // switches privilege modes with privileged instructions, which the
        // workload only draws when the ISA names them.
        ensure!(
            self.supports(instruction.opcode())
                || matches!(instruction.opcode().extension(), Extension::Raw | Extension::Privileged),
            "{} is not enabled for this target",
            instruction.opcode()
        );
        instruction.encode_for(self.xlen)?.append_bytes(bytes);
        Ok(())
    }
}
