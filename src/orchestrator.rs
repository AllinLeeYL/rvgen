use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use rand::{RngExt, SeedableRng, rngs::StdRng};

use crate::elf::{Elf, ElfSymbol, HostInterface, STT_OBJECT, write_executable};
use crate::entangle::{GOLDEN_SLOT_SIZE, code_size};
use crate::hart::Hart;
use crate::memory::{MemoryLayout, MemoryRegion, Permissions, Section};
use crate::options::{CommonOpts, ManyOpts, OneOpts};
use crate::riscv::Xlen;
use crate::spike::Spike;
use crate::target::Target;
use crate::weights::InstrWeights;

/// Data section holding the constants the code loads: the entanglement sites'
/// golden values, `_init`'s values and `_check`'s expected values.
const GOLDEN_SECTION: &str = ".golden";

/// Bounds of the code area's size, drawn log-uniformly per program as in
/// Cascade. Every block is within `jal` reach (1 MiB) of `_fail`.
const MIN_CODE_AREA: u64 = 16 << 10;
const MAX_CODE_AREA: u64 = 1 << 20;

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
    /// Whether the code loads its constants from `.golden` (RV64 only).
    golden_section: bool,
    /// The instruction mix of every basic block.
    weights: InstrWeights,
}

impl Orchestrator {
    fn new(
        target: Target,
        seed: u64,
        self_check: bool,
        entangle: Option<usize>,
        golden_section: bool,
        weights: InstrWeights,
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
            golden_section,
            weights,
        }
    }

    fn run(&mut self) -> Result<()> {
        for core in self.harts.iter_mut() {
            core.run(
                &mut self.rng,
                &self.target,
                &mut self.state,
                &self.weights,
                self.self_check,
                self.entangle,
                self.golden_section,
            )?;
        }
        // Golden values are only counted once the sites exist. Sections are
        // reserved downwards from the top of RAM, so this one lands below the
        // data the workload already points at.
        let slots = self.harts.first().map_or(0, Hart::golden_slots);
        if slots > 0 {
            let golden = self.state.memory.reserve(
                &self.target.physical_memory,
                GOLDEN_SECTION,
                slots as u64 * GOLDEN_SLOT_SIZE,
                GOLDEN_SLOT_SIZE,
                Permissions::RW,
            )?;
            // Workload accesses never target it, so only Spike's values land there.
            golden.private = true;
        }
        // The code area goes at the bottom of RAM, below the data reserved
        // from the top, at least four times the code so blocks find room within reach.
        if let Some(hart) = self.harts.first_mut() {
            let code: u64 = hart.bbs.iter().map(|bb| code_size(&bb.instrs) as u64).sum();
            let min = (4 * code).max(MIN_CODE_AREA);
            ensure!(
                min <= MAX_CODE_AREA,
                "{code} bytes of code need more than the {MAX_CODE_AREA}-byte code area; lower --num-instrs"
            );
            let ratio = (MAX_CODE_AREA as f64 / min as f64).powf(self.rng.random::<f64>());
            let size = (min as f64 * ratio) as u64 / 4 * 4;
            let start = self.target.physical_memory.start;
            let region = MemoryRegion { start, size, permissions: Permissions::RX };
            self.state.memory.add(Section {
                name: ".text".into(),
                alignment: 4,
                permissions: Permissions::RX,
                region,
                private: true,
            });
            let align = self.target.instruction_alignment() as u64;
            hart.place(start, size, align, &mut self.rng)?;
        }
        Ok(())
    }

    /// Encode the ELF image.
    fn encode(&self) -> Result<Vec<u8>> {
        // ELF output packages core 0's workload
        let Some(hart) = self.harts.first() else {
            return Ok(Vec::new());
        };
        let mut elf = Elf::new(&self.target, &self.state.memory)?;
        elf.add_code(&hart.image())?;
        if self.state.memory.get(GOLDEN_SECTION).is_ok() {
            let golden = hart.golden_bytes();
            let section = elf.fill(GOLDEN_SECTION, &golden)?;
            elf.add_symbol(ElfSymbol {
                name: "_golden".into(),
                section,
                offset: 0,
                size: golden.len() as u64,
                symbol_type: STT_OBJECT,
            })?;
        }
        elf.apply(HostInterface)?;
        elf.finish()
    }

    /// Fix the code's absolute addresses, which the code needs before it can
    /// run.
    fn link(&mut self) -> Result<()> {
        let golden_addr = self
            .state
            .memory
            .get(GOLDEN_SECTION)
            .ok()
            .map(|section| section.region.start);
        if let Some(hart) = self.harts.first_mut() {
            hart.link(golden_addr, self.target.xlen)?;
        }
        Ok(())
    }

    /// Resolve the self-check values against spike. 
    /// These values is only known after a spike run.
    /// This func invoke spike, retrieve these values, and patch the program.
    fn resolve_self_check(&mut self, spike: &Spike) -> Result<Vec<u8>> {
        let draft = self.encode()?;
        let hart = &mut self.harts[0];

        // PCs where spike should report its register state.
        let mut pcs = hart.probe_pcs(self.target.xlen);
        pcs.push(hart.self_check_pc()?);

        // Retrieve architecture state from goldem model -- spike
        let mut states = spike
            .states_at(&draft, &pcs)
            .context("cannot compute the self-check's expected values")?;
        let expected = states.pop().expect("the check is probed");
        
        // Patch data/control-flow entanglement sites
        hart.patch_entanglements(&states, &self.target, &mut self.rng)?;
        // Patch the self-check site at the end.
        hart.set_final_expected_values(&expected, &self.target)?;

        let image = self.encode()?;
        let code = spike.exit_code(&image)?;
        ensure!(
            code == 0,
            "the self-checking program fails on Spike itself (exit code {code})"
        );
        Ok(image)
    }
}

/// Generate one program return the ELF bytecode. Spike is run
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
    let golden_section = !opts.inline_golden && opts.xlen == Xlen::X64;
    let weights = opts.instr_weights().checked(&target, entangle.is_some())?;
    let spike = Spike::new(&opts.spike, &target);
    let seed = opts.seed.unwrap_or_else(rand::random);
    let mut orchestrator =
        Orchestrator::new(target, seed, self_check, entangle, golden_section, weights);

    // generate code -- it allocate memory address on the fly
    orchestrator.run()?;
    orchestrator.link()?;
    if self_check {
        orchestrator.resolve_self_check(&spike)
    } else {
        orchestrator.encode()
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
