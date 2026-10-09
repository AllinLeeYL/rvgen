use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use rand::{RngExt, SeedableRng, rngs::StdRng};

use crate::elf::{Elf, ElfSymbol, HostInterface, STT_OBJECT, write_executable};
use crate::entangle::{GOLDEN_SLOT_SIZE, code_size};
use crate::hart::Hart;
use crate::memory::{MemoryLayout, MemoryRegion, Permissions, Section};
use crate::options::{CommonOpts, ManyOpts, OneOpts};
use crate::privilege::{MAX_PMP_WINDOW_REGIONS, PMP_REGION};
use crate::riscv::{PrivilegeLevel, Xlen};
use crate::spike::Spike;
use crate::target::Target;
use crate::utils::log_uniform;
use crate::weights::InstrWeights;

/// Data section holding the constants the code loads: the entanglement sites'
/// golden values, `_init`'s values and `_check`'s expected values.
const GOLDEN_SECTION: &str = ".golden";

/// Bounds of the code area's size, drawn log-uniformly per program as in
/// Cascade. Every block is within `jal` reach (1 MiB) of `_fail`.
const MIN_CODE_AREA: u64 = 16 << 10;
const MAX_CODE_AREA: u64 = 1 << 20;

/// Bounds of the data section's size when `--scratch-size` is not given,
/// drawn the same way: from fitting in the L1 data cache to far exceeding it.
const MIN_SCRATCH: u64 = 4 << 10;
const MAX_SCRATCH: u64 = 1 << 20;

/// The data section the workload's memory accesses target.
const SCRATCH_SECTION: &str = "scratch";

/// The private data the PMP window covers (see [`crate::privilege`]).
pub const PMP_WINDOW_SECTION: &str = "pmpwin";

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
    /// Random initial contents of the data section, so loads see data.
    scratch: Vec<u8>,
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
        let ram = &target.physical_memory;
        let scratch_size = target
            .scratch_size
            .unwrap_or_else(|| log_uniform(MIN_SCRATCH, MAX_SCRATCH.min(ram.size / 4).max(MIN_SCRATCH), &mut rng) / 8 * 8);
        let scratch = match state.memory.reserve(ram, SCRATCH_SECTION, scratch_size, 8, Permissions::RW) {
            Ok(section) => {
                let mut bytes = vec![0; section.region.size as usize];
                rng.fill(&mut bytes[..]);
                bytes
            }
            Err(_) => Vec::new(),
        };
        let _ = state.memory.reserve(&target.physical_memory, "smc", target.smc_size, 4, Permissions::RWX);
        // Private 4 KiB regions the low PMP entries guard, for planned access
        // faults in S and U; the last entry stays for everything else.
        let regions = target.pmp_regions.saturating_sub(1).min(MAX_PMP_WINDOW_REGIONS);
        if target.has_privilege(PrivilegeLevel::User) && regions > 0 {
            let size = regions as u64 * PMP_REGION;
            if let Ok(section) = state.memory.reserve(ram, PMP_WINDOW_SECTION, size, PMP_REGION, Permissions::RW) {
                section.private = true;
            }
        }

        Self {
            target,
            harts,
            state,
            rng,
            self_check,
            entangle,
            golden_section,
            weights,
            scratch,
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
            let size = log_uniform(min, MAX_CODE_AREA, &mut self.rng) / 4 * 4;
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
        if !self.scratch.is_empty() {
            elf.fill(SCRATCH_SECTION, &self.scratch)?;
        }
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
    let mut target = Target::new(
        opts.xlen,
        opts.isa.iter().copied(),
        opts.privileges.iter().copied(),
        opts.disabled_instrs.iter().copied().collect(),
        opts.num_cores,
        opts.num_instrs,
        ram,
        opts.scratch_size,
        opts.smc_size,
        opts.pmp_regions,
    )?;
    target.medeleg_mask = opts.medeleg_mask;
    target.misaligned_traps = opts.misaligned_traps;
    
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
