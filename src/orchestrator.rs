use rand::Rng;
use std::fs;
use std::path::Path;

use crate::context::GenContext;
use crate::options::{ManyOpts, OneOpts};
// use crate::riscv::{Instruction, XReg};
use crate::basicblock::BasicBlock;
use crate::runtime::write_executable;
use crate::utils::cut_cake_randomly;

use anyhow::{Ok, Result};

#[derive(Default)]
struct Core {
    bbs: Vec<BasicBlock>,
}

impl Core {
    fn new(num_instrs: usize, rng: &mut (impl Rng + ?Sized)) -> Self {
        let core = Self {
            bbs: cut_cake_randomly(num_instrs, Some(1), Some(32), rng)
                .into_iter()
                .enumerate()
                .map(|(id, budget)| BasicBlock::new(id, budget))
                .collect(),
        };
        debug_assert_eq!(
            core.bbs.iter().map(|bb| bb.budget).sum::<usize>(),
            num_instrs,
        );
        core
    }

    fn run(&mut self, rng: &mut (impl Rng + ?Sized), ctx: &GenContext) -> Result<()> {
        for bb in self.bbs.iter_mut() {
            bb.run(rng, ctx)?;
        }
        Ok(())
    }

    fn encode(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        for bb in &self.bbs {
            bytes.extend_from_slice(&bb.encode()?);
        }
        Ok(bytes)
    }
}

struct Orchestrator<'a> {
    num_instrs: usize,
    cores: Vec<Core>,
    ctx: &'a GenContext,
}

impl<'a> Orchestrator<'a> {
    fn new(
        num_instrs: usize,
        num_cores: usize,
        rng: &mut (impl Rng + ?Sized),
        ctx: &'a GenContext,
    ) -> Self {
        Self {
            num_instrs: num_instrs,
            cores: std::iter::repeat_with(|| Core::new(num_instrs, rng))
                .take(num_cores)
                .collect(),
            ctx: ctx,
        }
    }

    fn run(&mut self, rng: &mut (impl Rng + ?Sized)) -> Result<()> {
        for core in self.cores.iter_mut() {
            core.run(rng, self.ctx)?;
        }
        Ok(())
    }

    fn encode(&self) -> Result<Vec<u8>> {
        if self.cores.is_empty() {
            return Ok(Vec::new());
        }
        // ELF output packages core 0's workload
        self.cores[0].encode()
    }
}

pub fn gen_one(opts: OneOpts, mkdir: bool) -> Result<()> {
    let ctx = GenContext::new(&opts);
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
        &ctx,
    );
    orchestrator.run(&mut rng)?;
    let bytes = orchestrator.encode()?;
    write_executable(&bytes, true, 0x8000_0000, Path::new(&opts.output))
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
