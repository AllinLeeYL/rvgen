use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use rand::{RngExt, SeedableRng, rngs::StdRng};

use crate::elf::{Elf, ElfSymbol, HostInterface, STT_OBJECT, write_executable};
use crate::entangle::{GOLDEN_SLOT_SIZE, code_size};
use crate::hart::Hart;
use crate::memory::{MemoryLayout, MemoryRegion, Permissions, Section};
use crate::options::{CommonOpts, ManyOpts, OneOpts};
use crate::privilege::{Cause, MAX_PMP_WINDOW_REGIONS, PMP_REGION};
use crate::riscv::{Csr, PrivilegeLevel, Xlen};
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
        let harts = std::iter::repeat_with(|| Hart::new(target.num_instrs, target.smc_proba, &mut rng))
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
            // Before any size is summed: the store sites grow the code.
            core.plan_smc(&self.target, &mut self.rng)?;
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
        // from the top, four times the code where it fits so blocks find room within reach.
        if let Some(hart) = self.harts.first_mut() {
            let code: u64 = hart.bbs.iter().map(|bb| code_size(&bb.instrs) as u64).sum();
            let min = (4 * code).max(MIN_CODE_AREA);
            // The code area ends where the data reserved from the top starts.
            let start = self.target.physical_memory.start;
            let data = self.state.memory.sections.iter().map(|section| section.region.start).min();
            let max = MAX_CODE_AREA.min(data.map_or(self.target.physical_memory.size, |data| data - start));
            let size = match self.target.code_area_size {
                Some(size) => {
                    ensure!(size <= max, "--code-area-size exceeds the {max} bytes left below the data");
                    ensure!(code <= size, "{code} bytes of code do not fit --code-area-size {size}");
                    size
                }
                None => {
                    // Large programs settle for less room, down to twice the code.
                    let min = if 2 * code <= max { min.min(max) } else { min };
                    ensure!(
                        min <= max,
                        "{code} bytes of code need more than the {max}-byte code area; \
                         lower --num-instrs (or --scratch-size on a small RAM)"
                    );
                    log_uniform(min, max, &mut self.rng)
                }
            } / 4 * 4;
            let region = MemoryRegion { start, size };
            self.state.memory.add(Section {
                name: ".text".into(),
                alignment: 4,
                permissions: Permissions::RWX,
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
            hart.link(golden_addr, &self.target)?;
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
    Ok(generate_with(opts)?.1)
}

/// [`generate`], also returning the generator's final state.
fn generate_with(opts: &CommonOpts) -> Result<(Orchestrator, Vec<u8>)> {
    let ram = MemoryRegion {
        start: opts.ram_base,
        size: opts.ram_size,
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
    target.code_area_size = opts.code_area_size;
    target.medeleg_mask = opts.medeleg_mask;
    target.misaligned_traps = opts.misaligned_traps;
    target.smc_aligned_stores = opts.smc_aligned_stores;
    ensure!(!opts.tvec_modes.is_empty(), "--tvec-modes needs direct, vectored or both");
    target.tvec_modes = opts.tvec_modes.iter().copied().collect();
    for (flag, align) in [("--tvec-align", opts.tvec_align), ("--vectored-tvec-align", opts.vectored_tvec_align)] {
        ensure!(align >= 4 && align.is_power_of_two(), "{flag} must be a power of two, at least 4");
    }
    target.tvec_align = opts.tvec_align;
    target.vectored_tvec_align = opts.vectored_tvec_align;
    for (flag, proba) in [
        ("--vectored-proba", opts.vectored_proba),
        ("--switch-proba", opts.switch_proba),
        ("--config-proba", opts.config_proba),
    ] {
        ensure!(proba.is_none_or(|proba| (0.0..=1.0).contains(&proba)), "{flag} must lie in 0..=1");
    }
    target.vectored_proba = opts.vectored_proba;
    target.switch_proba = opts.switch_proba;
    target.config_proba = opts.config_proba;
    for &(cause, weight) in &opts.trap_weights {
        ensure!(weight >= 0.0 && weight.is_finite(), "--trap-weight needs a non-negative weight");
        // The only traps sure to bring S and U code back to M before `_check`.
        ensure!(
            weight > 0.0 || !matches!(cause, Cause::EcallU | Cause::EcallS),
            "--trap-weight cannot exclude ecall-u or ecall-s: they return the hart to M"
        );
        target.trap_weights[cause as usize] = Some(weight);
    }
    target.disabled_csrs = opts.disabled_csrs.clone();
    target.disabled_illegal = opts.disabled_illegal.clone();
    for csr in [Csr::MSCRATCH, Csr::MEPC, Csr::MCAUSE, Csr::SEPC, Csr::SCAUSE] {
        ensure!(target.has_csr(csr), "--disabled-csrs cannot name {csr}: the generated code relies on it");
    }
    ensure!((0.0..=1.0).contains(&opts.smc_proba), "--smc-proba must lie in 0..=1");
    target.smc_proba = opts.smc_proba;
    
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
    let image = if self_check {
        orchestrator.resolve_self_check(&spike)?
    } else {
        orchestrator.encode()?
    };
    Ok((orchestrator, image))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::riscv::Extension::{A, C, D, F, I, M, Zicsr, Zifencei};

    /// SMC programs pass on Spike (checked by `generate`), and every SMC
    /// block's code is absent from the image: stale bytes sit at its address.
    #[test]
    fn smc_blocks_are_stored_at_run_time() {
        let (mut fresh, mut aliased, mut misaligned) = (0, 0, 0);
        for (seed, isa) in (0..12).zip([vec![I, M, C, Zicsr, Zifencei], vec![I, M, A, Zicsr, Zifencei]].iter().cycle()) {
            let mut opts = CommonOpts::default();
            opts.isa = isa.clone();
            opts.num_instrs = 2000;
            opts.smc_proba = 0.3;
            opts.seed = Some(seed);
            let (orchestrator, _) = generate_with(&opts).unwrap();
            let hart = &orchestrator.harts[0];
            let target = &orchestrator.target;
            let text = hart.image().iter().map(|bb| bb.encode(target).unwrap()).collect::<Vec<_>>().concat();
            let start = target.physical_memory.start;
            for bb in hart.bbs.iter().filter(|bb| bb.is_smc) {
                let code = bb.encode(target).unwrap();
                let at = (bb.addr - start) as usize;
                assert_ne!(&text[at..at + code.len()], &code[..], "SMC block shipped in the image");
                if bb.alias.is_some() { aliased += 1 } else { fresh += 1 }
            }
            // Misaligned stores do not trap by default, so some are.
            for store in hart.bbs.iter().flat_map(|bb| &bb.smc_stores) {
                let mut addr = hart.bbs[store.smc].addr as usize;
                for width in &store.widths {
                    misaligned += (addr % width != 0) as usize;
                    addr += width;
                }
            }
        }
        assert!(fresh > 0 && aliased > 0 && misaligned > 0, "fresh {fresh}, aliased {aliased}, misaligned {misaligned}");
    }

    /// `--smc-aligned-stores` keeps every SMC store naturally aligned.
    #[test]
    fn smc_stores_can_stay_aligned() {
        let mut opts = CommonOpts::default();
        opts.isa = vec![I, M, C, Zicsr, Zifencei];
        opts.num_instrs = 2000;
        opts.smc_proba = 0.3;
        opts.smc_aligned_stores = true;
        opts.seed = Some(0);
        let (orchestrator, _) = generate_with(&opts).unwrap();
        let hart = &orchestrator.harts[0];
        let mut stores = 0;
        for store in hart.bbs.iter().flat_map(|bb| &bb.smc_stores) {
            let mut addr = hart.bbs[store.smc].addr as usize;
            for width in &store.widths {
                assert_eq!(addr % width, 0, "misaligned SMC store");
                addr += width;
                stores += 1;
            }
        }
        assert!(stores > 0, "no SMC store");
    }

    /// A vectored-only target (Ibex) reaches every landing and sink handler
    /// through a 256-byte aligned BASE, and never sees a disabled CSR or cause.
    #[test]
    fn target_options_shape_the_traps() {
        use crate::privilege::TvecMode;
        let mut opts = CommonOpts::default();
        opts.isa = vec![I, M, C, Zicsr];
        opts.xlen = Xlen::X32;
        opts.privileges = vec![PrivilegeLevel::Machine, PrivilegeLevel::User];
        opts.pmp_regions = 0;
        opts.tvec_modes = vec![TvecMode::Vectored];
        opts.switch_proba = Some(0.3);
        opts.trap_weights = vec![("breakpoint".parse().unwrap(), 0.0)];
        opts.disabled_csrs = vec!["mhpmevent".parse().unwrap(), "0x320".parse().unwrap()];
        opts.disabled_illegal = vec!["cycle-write".parse().unwrap()];
        opts.seed = Some(1);
        let (orchestrator, _) = generate_with(&opts).unwrap();
        let hart = &orchestrator.harts[0];
        let landings: Vec<_> = hart.bbs.iter().filter(|bb| bb.landing.is_some()).collect();
        assert!(landings.len() > 2, "no planned trap");
        assert!(landings.iter().all(|bb| bb.addr % 256 == 0));
        let text = hart.bbs.iter().flat_map(|bb| &bb.instrs).map(|instr| format!("{instr:?}")).collect::<String>();
        assert!(text.contains("Ecall") && text.contains("Csr(832)"), "the text shows instructions and CSRs");
        assert!(!text.contains("Ebreak") && !text.contains("Csr(3072)"), "a disabled trap is raised");
        assert!(!(0x320..=0x33f).any(|csr| text.contains(&format!("Csr({csr})"))), "a disabled CSR is accessed");
        assert!("mhpmcounter32".parse::<crate::csrs::CsrRange>().is_err());
    }

    /// Options of the BOOM campaign that found the stale probes.
    fn smc_opts(seed: u64) -> CommonOpts {
        let mut opts = CommonOpts::default();
        opts.isa = vec![I, M, A, F, D, C, Zicsr, Zifencei];
        opts.num_instrs = 4096;
        opts.smc_proba = 0.3;
        opts.medeleg_mask = 0xb15d;
        opts.misaligned_traps = true;
        opts.seed = Some(seed);
        opts
    }

    /// The final program of `seed` passes on Spike, run here once more.
    fn passes_on_spike(seed: u64) -> Result<()> {
        let opts = smc_opts(seed);
        let (orchestrator, image) = generate_with(&opts)?;
        let code = Spike::new(&opts.spike, &orchestrator.target).exit_code(&image)?;
        ensure!(code == 0, "exit code {code}");
        Ok(())
    }

    /// Seeds whose SMC block has a probe at an address its old block ran at
    /// with other registers: Spike must report the SMC block's pass.
    #[test]
    fn smc_probes_skip_the_old_block() {
        for seed in [7147, 15773, 20180, 20872] {
            passes_on_spike(seed).unwrap_or_else(|error| panic!("seed {seed}: {error:#}"));
        }
    }

    /// Many SMC programs pass on Spike. Slow: `mise run test-smc`.
    #[test]
    #[ignore]
    fn smc_programs_pass_on_spike() {
        let seeds = std::sync::atomic::AtomicU64::new(0);
        let failures = std::sync::Mutex::new(Vec::new());
        std::thread::scope(|scope| {
            for _ in 0..std::thread::available_parallelism().map_or(1, usize::from) {
                scope.spawn(|| loop {
                    let seed = seeds.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if seed >= 30000 {
                        break;
                    }
                    if let Err(error) = passes_on_spike(seed) {
                        failures.lock().unwrap().push(format!("seed {seed}: {error:#}"));
                    }
                });
            }
        });
        let failures = failures.into_inner().unwrap();
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
