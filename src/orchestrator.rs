use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use rand::{SeedableRng, rngs::StdRng};

use crate::elf::{Elf, HostInterface, write_executable};
use crate::hart::Hart;
use crate::memory::{MemoryLayout, MemoryRegion, Permissions, Section};
use crate::options::{CommonOpts, ManyOpts, OneOpts};
use crate::spike::Spike;
use crate::target::Target;
use crate::weights::{BlockContext, Fixed, WeightPolicy};

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
    /// Decides each basic block's instruction mix.
    weights: Box<dyn WeightPolicy>,
}

impl Orchestrator {
    fn new(
        target: Target,
        seed: u64,
        self_check: bool,
        entangle: Option<usize>,
        weights: Box<dyn WeightPolicy>,
    ) -> Self {
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
            weights,
        }
    }

    fn run(&mut self) -> Result<()> {
        for (hart, core) in self.harts.iter_mut().enumerate() {
            let weights: Vec<_> = core
                .bbs
                .iter()
                .enumerate()
                .map(|(block, bb)| {
                    let ctx = BlockContext {
                        hart,
                        block,
                        budget: bb.budget,
                    };
                    self.weights.draw(&ctx, &mut self.rng)
                })
                .collect();
            core.run(
                &mut self.rng,
                &self.target,
                &mut self.state,
                &weights,
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

/// Generate one program for `opts` and return the ELF image. Spike is run
/// here when the program self-checks.
pub fn generate(opts: &CommonOpts) -> Result<Vec<u8>> {
    let ram = MemoryRegion {
        start: opts.ram_base,
        size: opts.ram_size,
        permissions: Permissions::RWX,
    };
    let target = Target::new(
        opts.xlen,
        opts.isa.iter().copied(),
        opts.privileges.iter().copied(),
        opts.disabled_instrs.iter().copied().collect(),
        opts.num_cores,
        opts.num_instrs,
        ram,
        opts.scratch_size,
        opts.smc_size,
    )?;
    let self_check = !opts.no_self_check;
    let entangle = (self_check && !opts.no_entangle).then_some(opts.guard_threshold);
    let weights = opts.weight_spec().build(&target, entangle.is_some())?;
    let spike = Spike::new(&opts.spike, &target);
    let seed = opts.seed.unwrap_or_else(rand::random);
    let mut orchestrator =
        Orchestrator::new(target, seed, self_check, entangle, Box::new(Fixed(weights)));

    // generate code -- it allocate memory address on the fly
    orchestrator.run()?;
    if self_check {
        orchestrator.resolve_self_check(&spike)
    } else {
        Ok(orchestrator.encode()?.0)
    }
}

pub fn gen_one(opts: OneOpts, mkdir: bool) -> Result<()> {
    if mkdir {
        if let Some(parent) = Path::new(&opts.output).parent() {
            fs::create_dir_all(parent)?;
        }
    }
    let bytes = generate(&opts.common)?;
    write_executable(&bytes, Path::new(&opts.output))
}

pub fn gen_many(opts: ManyOpts) -> Result<()> {
    fs::create_dir_all(&opts.outdir)?;
    for i in 0..opts.num_elfs {
        let mut common = opts.common.clone();
        common.seed = common.seed.map(|seed| seed.wrapping_add(u64::from(i)));
        let oneopts = OneOpts {
            common,
            output: format!("{}/{}.elf", opts.outdir, i),
        };
        gen_one(oneopts, false)?;
    }
    Ok(())
}
