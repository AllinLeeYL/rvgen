//! Parameters for the generator, cores, and basic blocks.

use crate::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratorParams {
    /// Total emitted workload instructions across all cores and blocks.
    /// Runtime entry/exit instructions are excluded. Zero is an empty workload.
    pub size: usize,
    pub memsize: usize,
    pub num_cores: usize,
    pub num_bbs: usize,
    pub seed: i64,
    pub authorize_privileges: bool,
    pub is_64bit: bool,
    pub start_addr: u64,
}

impl Default for GeneratorParams {
    fn default() -> Self {
        Self {
            size: 256,
            memsize: 4096,
            num_cores: 1,
            num_bbs: 12,
            seed: 0,
            authorize_privileges: true,
            is_64bit: true,
            start_addr: 0x8000_0000,
        }
    }
}

impl GeneratorParams {
    pub fn validate(&self) -> Result<()> {
        if self.num_cores == 0 {
            return Err("num_cores must be greater than zero".into());
        }
        if self.num_bbs == 0 {
            return Err("num_bbs must be greater than zero".into());
        }
        if !self.is_64bit && self.start_addr > u32::MAX as u64 {
            return Err("start_addr does not fit ELF32".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct BasicBlockGeneratorParams {
    /// Emitted instruction budget for this block, including action sequences.
    pub num_insts: usize,
    pub label: String,
}

impl Default for BasicBlockGeneratorParams {
    fn default() -> Self {
        Self {
            num_insts: 8,
            label: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CoreGeneratorParams {
    pub num_bbs: usize,
    /// Total instruction budget for this core, divided across its blocks.
    pub num_insts: usize,
}

/// Distribute a total without dropping the remainder or multiplying it.
pub(super) fn partition(total: usize, parts: usize, index: usize) -> usize {
    total / parts + usize::from(index < total % parts)
}

impl Default for CoreGeneratorParams {
    fn default() -> Self {
        Self {
            num_bbs: 1,
            num_insts: 20,
        }
    }
}
