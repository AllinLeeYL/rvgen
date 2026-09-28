use rand::Rng;
use std::fs;
use std::path::Path;

use crate::basicblock::BasicBlock;
use crate::options::{ManyOpts, OneOpts};
use crate::runtime::memory::MemoryPlan;
use crate::runtime::{Runtime, Target};
use crate::utils::cut_cake_randomly;

use anyhow::{Ok, Result};

#[derive(Default)]
struct Core {
    bbs: Vec<BasicBlock>,
}

impl Core {
    fn new(num_instrs: usize, rng: &mut (impl Rng + ?Sized)) -> Self {
        // Randomly distribute budgets to basic blocks
        let core = Self {
            bbs: cut_cake_randomly(num_instrs, Some(1), Some(32), rng)
                .into_iter()
                .enumerate()
                .map(|(id, budget)| BasicBlock::new(id, rand::random::<bool>(), budget))
                .collect(),
        };
        debug_assert_eq!(
            core.bbs.iter().map(|bb| bb.budget).sum::<usize>(),
            num_instrs,
        );
        core
    }

    fn run(&mut self, rng: &mut (impl Rng + ?Sized), runtime: &Runtime) -> Result<()> {
        for bb in self.bbs.iter_mut() {
            bb.run(rng, runtime)?;
        }
        Ok(())
    }

    fn encode(&self, target: &Target) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        for bb in &self.bbs {
            bytes.extend_from_slice(&bb.encode(target)?);
        }
        Ok(bytes)
    }
}

struct Orchestrator<'a> {
    cores: Vec<Core>,
    runtime: &'a Runtime,
}

impl<'a> Orchestrator<'a> {
    fn new(
        num_instrs: usize,
        num_cores: usize,
        rng: &mut (impl Rng + ?Sized),
        runtime: &'a Runtime,
    ) -> Self {
        Self {
            cores: std::iter::repeat_with(|| Core::new(num_instrs, rng))
                .take(num_cores)
                .collect(),
            runtime,
        }
    }

    fn run(&mut self, rng: &mut (impl Rng + ?Sized)) -> Result<()> {
        for core in self.cores.iter_mut() {
            core.run(rng, self.runtime)?;
        }
        Ok(())
    }

    fn encode(&self) -> Result<Vec<u8>> {
        if self.cores.is_empty() {
            return Ok(Vec::new());
        }
        // ELF output packages core 0's workload
        self.cores[0].encode(self.runtime.target())
    }
}

pub fn gen_one(opts: OneOpts, mkdir: bool) -> Result<()> {
    let target = Target::new(opts.common.xlen, opts.common.isa.iter().copied())?;
    let memory = MemoryPlan::bare_metal(
        opts.common.ram_base,
        opts.common.ram_size,
        opts.common.scratch_size,
        opts.common.smc_size,
    )?;
    let runtime = Runtime::new(target, memory, opts.common.disabled_instrs.iter().copied())?;
    if mkdir {
        if let Some(parent) = Path::new(&opts.output).parent() {
            fs::create_dir_all(parent)?;
        }
    }
    let mut rng = rand::rng();
    let mut orchestrator = Orchestrator::new(
        opts.common.num_instrs,
        opts.common.num_cores,
        &mut rng,
        &runtime,
    );
    orchestrator.run(&mut rng)?;
    let bytes = orchestrator.encode()?;
    runtime.write_executable(&bytes, Path::new(&opts.output))
}

pub fn gen_many(opts: ManyOpts) -> Result<()> {
    fs::create_dir_all(&opts.outdir)?;
    for i in 0..opts.num_elfs {
        let oneopts = OneOpts {
            common: opts.common.clone(),
            output: format!("{}/{}.elf", opts.outdir, i),
        };
        gen_one(oneopts, false)?;
    }
    Ok(())
}
