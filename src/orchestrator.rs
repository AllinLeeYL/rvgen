use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use rand::{RngExt, SeedableRng, rngs::StdRng};

use crate::elf::{Elf, HostInterface, write_executable};
use crate::hart::Hart;
use crate::memory::{MemoryLayout, MemoryRegion, Permissions, Section};
use crate::options::{ManyOpts, OneOpts};
use crate::spike::Spike;
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
    self_check: bool,
    /// The mid-block guard threshold when entangling.
    entangle: Option<usize>,
}

impl Orchestrator {
    fn new(target: Target, seed: u64, self_check: bool, entangle: Option<usize>) -> Self {
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
        let _ = state.memory.reserve(&target.physical_memory, "scratch", target.scratch_size, 4, Permissions::RW);
        let _ = state.memory.reserve(&target.physical_memory, "smc", target.smc_size, 4, Permissions::RWX);

        Self {
            target,
            harts,
            state,
            rng,
            self_check,
            entangle,
        }
    }

    fn run(&mut self) -> Result<()> {
        for core in self.harts.iter_mut() {
            core.run(
                &mut self.rng,
                &self.target,
                &mut self.state,
                self.self_check,
                self.entangle,
            )?;
        }
        self.merge_memory_sections();
        Ok(())
    }

    fn merge_memory_sections(&mut self) -> Result<()> {
        Ok(())
    }

    /// Encode the ELF image and return it with the address of core 0's code.
    fn encode(&self) -> Result<(Vec<u8>, u64)> {
        // ELF output packages core 0's workload
        let Some(hart) = self.harts.first() else {
            return Ok((Vec::new(), 0));
        };
        let mut elf = Elf::new(&self.target, &self.state.memory)?;
        elf.add_code(&hart.bbs)?;
        let text = elf.section_index(".text").expect("add_code places .text");
        let text_addr = elf.sections()[text].addr;
        elf.apply(HostInterface)?;
        Ok((elf.finish()?, text_addr))
    }

    /// Resolve the self-check against Spike: run the placeholder program
    /// through every entanglement site up to the check, patch the observed
    /// registers into the sites and in as the check's expected values, and
    /// confirm the final program passes on Spike. Returns the final image.
    fn resolve_self_check(&mut self, spike: &Spike) -> Result<Vec<u8>> {
        // Sites need absolute code addresses before the draft can run.
        let (_, text_addr) = self.encode()?;
        self.harts[0].link(text_addr, self.target.xlen)?;
        let (draft, draft_text_addr) = self.encode()?;
        ensure!(draft_text_addr == text_addr, "code moved while linking sites");

        let hart = &mut self.harts[0];
        let mut pcs: Vec<u64> = hart
            .probe_offsets(self.target.xlen)
            .into_iter()
            .map(|offset| text_addr + offset as u64)
            .collect();
        pcs.push(text_addr + hart.check_offset()? as u64);
        let mut states = spike
            .states_at(&draft, &pcs)
            .context("cannot compute the self-check's expected values")?;
        let expected = states.pop().expect("the check is probed");
        hart.resolve_sites(&states, &self.target, &mut self.rng)?;
        hart.set_expected(&expected, &self.target)?;

        let (image, final_text_addr) = self.encode()?;
        ensure!(final_text_addr == text_addr, "code moved while patching the self-check");
        let code = spike.exit_code(&image)?;
        ensure!(
            code == 0,
            "the self-checking program fails on Spike itself (exit code {code})"
        );
        Ok(image)
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
        opts.common.privileges.iter().copied(),
        opts.common.disabled_instrs.iter().copied().collect(),
        opts.common.num_cores,
        opts.common.num_instrs,
        ram.clone(),
        opts.common.scratch_size,
        opts.common.smc_size,
    )?;
    if mkdir {
        if let Some(parent) = Path::new(&opts.output).parent() {
            fs::create_dir_all(parent)?;
        }
    }
    let self_check = !opts.common.no_self_check;
    let entangle = (self_check && !opts.common.no_entangle).then_some(opts.common.guard_threshold);
    let spike = Spike::new(&opts.common.spike, &target);
    let mut rng = rand::rng();
    let mut orchestrator = Orchestrator::new(target, rng.random(), self_check, entangle);

    // generate code -- it allocate memory address on the fly
    orchestrator.run()?;
    let bytes = if self_check {
        orchestrator.resolve_self_check(&spike)?
    } else {
        orchestrator.encode()?.0
    };
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
