use std::fs;
use std::path::Path;

use anyhow::{Result, ensure};
use rand::{RngExt, SeedableRng, rngs::StdRng};

use crate::elf::{Elf, HostInterface, write_executable};
use crate::hart::Hart;
use crate::memory::{MemoryLayout, MemoryRegion, Permissions, Section};
use crate::options::{ManyOpts, OneOpts};
use crate::target::Target;

#[derive(Default)]
pub struct GlobalState {
    pub memory: MemoryLayout,
}

pub struct Orchestrator {
    target: Target, // target remains constant during generation.
    harts: Vec<Hart>,
    state: GlobalState, // Everything that may change during runtime is defined here.
    rng: StdRng,
}

impl Orchestrator {
    fn new(target: Target, seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        let harts = std::iter::repeat_with(|| Hart::new(target.num_instrs, &mut rng))
            .take(target.num_cores)
            .collect();

        let mut state = GlobalState::default();
        // The HTIF mailboxes are placed up front so the exit and trap code can
        // address `tohost` directly; [`HostInterface`] reuses them.
        for name in [".tohost", ".fromhost"] {
            if let Ok(section) = state.memory.reserve(
                &target.physical_memory,
                name,
                HostInterface::SIZE,
                HostInterface::ALIGNMENT,
                Permissions::RW,
            ) {
                section.private = true;
            }
        }
        let _ = state.memory.reserve(&target.physical_memory, "scratch", 4096, 4, Permissions::RW);
        let _ = state.memory.reserve(&target.physical_memory, "smc", 4096, 4, Permissions::RWX);

        Self {
            target,
            harts,
            state,
            rng,
        }
    }

    fn run(&mut self) -> Result<()> {
        for core in self.harts.iter_mut() {
            core.run(&mut self.rng, &self.target, &mut self.state)?;
        }
        self.merge_memory_sections();
        Ok(())
    }

    fn merge_memory_sections(&mut self) -> Result<()> {
        Ok(())
    }

    fn encode(&self) -> Result<Vec<u8>> {
        // ELF output packages core 0's workload
        let Some(hart) = self.harts.first() else {
            return Ok(Vec::new());
        };
        let mut elf = Elf::new(&self.target, &self.state.memory)?;
        elf.add_code(&hart.bbs)?;
        elf.apply(HostInterface)?;
        elf.finish()
    }
}

pub fn gen_one(opts: OneOpts, mkdir: bool) -> Result<()> {
    let ram = MemoryRegion {
        start: opts.common.ram_base,
        size: opts.common.ram_size,
        permissions: Permissions::RWX,
    };
    let target = Target::new(
        opts.common.xlen,
        opts.common.isa.iter().copied(),
        opts.common.disabled_instrs.iter().copied().collect(),
        opts.common.num_cores,
        opts.common.num_instrs,
        ram.clone(),
    )?;
    if mkdir {
        if let Some(parent) = Path::new(&opts.output).parent() {
            fs::create_dir_all(parent)?;
        }
    }
    let mut rng = rand::rng();
    let mut orchestrator = Orchestrator::new(target, rng.random());

    // generate code -- it allocate memory address on the fly
    orchestrator.run()?;
    let bytes = orchestrator.encode()?;
    write_executable(&bytes, Path::new(&opts.output))
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
