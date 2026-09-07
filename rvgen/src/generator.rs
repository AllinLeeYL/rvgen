//! Generator/core/basic-block hierarchy from the Python implementation.
//!
//! `generate` samples instruction classes. The original project does not yet
//! turn those selections into instructions. Callers can explicitly populate
//! each block's `insts` using the encoders in [`crate::riscv`].

use std::{fmt, path::Path};

use rand::{
    distributions::{Distribution, WeightedIndex},
    rngs::StdRng,
    SeedableRng,
};

use crate::{
    instgen::{ISAInstrClass, BASE_DISTRIBUTION},
    utils::elfbuilder::ElfBuilder,
    Result,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratorParams {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instruction {
    Standard(u32),
    Compressed(u16),
}

impl Instruction {
    fn append_bytes(self, bytes: &mut Vec<u8>) {
        match self {
            Self::Standard(word) => bytes.extend_from_slice(&word.to_le_bytes()),
            Self::Compressed(halfword) => bytes.extend_from_slice(&halfword.to_le_bytes()),
        }
    }
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Standard(word) => write!(f, ".4byte 0x{word:08x}"),
            Self::Compressed(halfword) => write!(f, ".2byte 0x{halfword:04x}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BasicBlockGeneratorParams {
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

#[derive(Debug, Clone)]
pub struct BasicBlockGenerator {
    pub params: BasicBlockGeneratorParams,
    pub insts: Vec<Instruction>,
    /// Retained for inspection; the Python scaffold discarded these selections.
    pub selected_classes: Vec<ISAInstrClass>,
}

impl BasicBlockGenerator {
    pub fn new(params: BasicBlockGeneratorParams) -> Self {
        Self {
            params,
            insts: Vec::new(),
            selected_classes: Vec::new(),
        }
    }

    pub fn generate(&mut self, prng: &mut StdRng, weights: &[(ISAInstrClass, f64)]) -> Result<()> {
        let distribution = class_distribution(weights)?;
        self.sample(prng, weights, &distribution);
        Ok(())
    }

    fn sample(
        &mut self,
        prng: &mut StdRng,
        weights: &[(ISAInstrClass, f64)],
        distribution: &WeightedIndex<f64>,
    ) {
        self.selected_classes.clear();
        self.selected_classes
            .extend((0..self.params.num_insts).map(|_| weights[distribution.sample(prng)].0));
    }
}

fn class_distribution(weights: &[(ISAInstrClass, f64)]) -> Result<WeightedIndex<f64>> {
    let total: f64 = weights.iter().map(|(_, w)| w).sum();
    if weights.iter().any(|(_, w)| !w.is_finite() || *w < 0.0) || !total.is_finite() || total <= 0.0
    {
        return Err(
            "instruction weights must be finite, nonnegative, and have a positive total".into(),
        );
    }
    Ok(WeightedIndex::new(weights.iter().map(|(_, w)| *w))?)
}

#[derive(Debug, Clone, Copy)]
pub struct CoreGeneratorParams {
    pub num_bbs: usize,
    pub num_insts: usize,
}

impl Default for CoreGeneratorParams {
    fn default() -> Self {
        Self {
            num_bbs: 1,
            num_insts: 20,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CoreGenerator {
    pub params: CoreGeneratorParams,
    pub bbs: Vec<BasicBlockGenerator>,
}

impl CoreGenerator {
    pub fn new(params: CoreGeneratorParams) -> Self {
        Self {
            params,
            bbs: (0..params.num_bbs)
                .map(|i| {
                    BasicBlockGenerator::new(BasicBlockGeneratorParams {
                        num_insts: params.num_insts,
                        label: format!("bb_{i}"),
                    })
                })
                .collect(),
        }
    }

    pub fn generate(&mut self, prng: &mut StdRng, weights: &[(ISAInstrClass, f64)]) -> Result<()> {
        let distribution = class_distribution(weights)?;
        for bb in &mut self.bbs {
            bb.sample(prng, weights, &distribution);
        }
        Ok(())
    }

    pub fn get_bytecode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for instruction in self.bbs.iter().flat_map(|bb| &bb.insts) {
            instruction.append_bytes(&mut bytes);
        }
        bytes
    }

    pub fn get_section_addr(&self) -> u64 {
        0
    }
}

#[derive(Debug)]
pub struct Generator {
    pub params: GeneratorParams,
    pub cores: Vec<CoreGenerator>,
    /// Weights belong to this instance, avoiding the Python module's global mutation.
    pub inst_weights: Vec<(ISAInstrClass, f64)>,
}

impl Generator {
    pub fn new(params: GeneratorParams) -> Result<Self> {
        params.validate()?;
        let cores = (0..params.num_cores)
            .map(|_| {
                CoreGenerator::new(CoreGeneratorParams {
                    num_bbs: params.num_bbs,
                    num_insts: params.size / params.num_cores,
                })
            })
            .collect();
        let mut inst_weights = BASE_DISTRIBUTION.to_vec();
        if params.num_cores == 1 {
            for (class, weight) in &mut inst_weights {
                if matches!(
                    class,
                    ISAInstrClass::SEND_IPI
                        | ISAInstrClass::CLEAR_INTERRUPT
                        | ISAInstrClass::WFI_TRAP
                        | ISAInstrClass::WFI_NOTRAP
                ) {
                    *weight = 0.0;
                }
            }
        }
        Ok(Self {
            params,
            cores,
            inst_weights,
        })
    }

    pub fn generate(&mut self) -> Result<()> {
        let mut prng = StdRng::seed_from_u64(self.params.seed.unsigned_abs());
        for core in &mut self.cores {
            core.generate(&mut prng, &self.inst_weights)?;
        }
        Ok(())
    }

    pub fn assembly(&self) -> Result<String> {
        let core = self.cores.first().ok_or("no cores to render")?;
        let mut blocks = String::new();
        for bb in &core.bbs {
            blocks.push_str(&format!("\n{}:\n    ", bb.params.label));
            for instruction in &bb.insts {
                blocks.push_str(&format!("\n    {instruction}\n    "));
            }
            blocks.push('\n');
        }
        Ok(include_str!("../templates/riscv64.S").replace("{{ basic_blocks }}", &blocks))
    }

    pub fn gen_assembly(&self, output_path: impl AsRef<Path>) -> Result<()> {
        std::fs::write(output_path, self.assembly()?)?;
        Ok(())
    }

    pub fn elf(&self) -> Result<Vec<u8>> {
        if self.cores.len() != 1 {
            return Err("only one core is supported for ELF output".into());
        }
        crate::runtime::executable(
            &self.cores[0].get_bytecode(),
            self.params.is_64bit,
            self.params.start_addr,
        )
    }

    pub fn gen_elf(&self, output_path: impl AsRef<Path>) -> Result<()> {
        ElfBuilder.save(&self.elf()?, output_path)
    }
}
