//! Generator orchestration and output for weighted instruction workloads.
//!
//! Policy selects an [`InstructionClass`] or [`GenerationAction`]. Lowering
//! supplies operands and emits complete [`Instruction`] sequences into blocks;
//! encoding occurs only when output is requested.

use std::path::Path;

use rand::{rngs::StdRng, SeedableRng};

use crate::{elf::ElfBuilder, Result};

mod lowering;
mod params;
pub mod policy;
mod program;

pub use crate::riscv::{Instruction, InstructionClass, InstructionKind};
pub use lowering::GenerationContext;
pub use params::{BasicBlockGeneratorParams, CoreGeneratorParams, GeneratorParams};
pub use policy::{GenerationAction, GenerationChoice};
pub use program::{BasicBlockGenerator, CoreGenerator, SelectedChoice};

#[derive(Debug)]
pub struct Generator {
    pub params: GeneratorParams,
    pub cores: Vec<CoreGenerator>,
    /// Weights belong to this instance; generators do not mutate each other's weights.
    pub choice_weights: Vec<(GenerationChoice, f64)>,
}

impl Generator {
    pub fn new(params: GeneratorParams) -> Result<Self> {
        params.validate()?;
        let cores = Self::create_cores(&params);
        let choice_weights =
            policy::default_choice_weights(params.is_64bit, params.authorize_privileges);
        Ok(Self {
            params,
            cores,
            choice_weights,
        })
    }

    fn create_cores(params: &GeneratorParams) -> Vec<CoreGenerator> {
        (0..params.num_cores)
            .map(|i| {
                CoreGenerator::new(CoreGeneratorParams {
                    num_bbs: params.num_bbs,
                    num_insts: params::partition(params.size, params.num_cores, i),
                })
            })
            .collect()
    }

    /// Regenerate the layout and workload from current parameters and weights.
    /// On error the previous cores and their instructions are retained.
    pub fn generate(&mut self) -> Result<()> {
        self.params.validate()?;
        let mut prng = StdRng::seed_from_u64(self.params.seed.unsigned_abs());
        let context = GenerationContext::from(&self.params);
        let mut cores = Self::create_cores(&self.params);
        for core in &mut cores {
            core.generate(&mut prng, &self.choice_weights, &context)?;
        }
        self.cores = cores;
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
        Ok(include_str!("../../templates/riscv64.S").replace("{{ basic_blocks }}", &blocks))
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
