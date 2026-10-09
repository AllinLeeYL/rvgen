//! Which CSRs the generated code touches, and how.
//!
//! The hart stays in M-mode for the whole program, and no CSR access may
//! change what an M-mode instruction does. Generated code therefore never
//! writes `mstatus` (beyond `_init` enabling the FPU), `mtvec`, `misa`,
//! `satp`, the interrupt enables or pendings, the debug triggers, or a PMP
//! entry's lock bit, which would also bind M-mode. Everything else here only
//! matters once a trap is delegated or the hart leaves M-mode, neither of which
//! happens, but it still drives the hardware that implements those CSRs.
//!
//! A CSR only exists when the target has what it belongs to: the S-mode CSRs
//! and `medeleg`/`mideleg` need S, `mcounteren` needs U, and the FP CSRs need F.
//! The workload only accesses those its current mode may (see
//! [`crate::privilege`]). With S or U, the trap vectors, `medeleg` and the PMP
//! are the privilege switches' to set: `stvec` points at `_strap_handler`
//! unless armed, `medeleg` holds the generator's value, and the PMP gives S
//! and U all memory but the PMP window.
//!
//! The self-check compares registers against Spike, so a CSR whose value read
//! back may differ between Spike and a correct core (WARL fields,
//! implementation-defined counters, unimplemented HPM counters reading zero)
//! is [`Read::Clobbered`]: the workload may read it, but overwrites the result
//! right away.
use anyhow::{Result, ensure};
use rand::distr::{Distribution, weighted::WeightedIndex};
use rand::{Rng, RngExt};

use crate::privilege::{PMP_REGION, PmpWindow, PrivState};
use crate::riscv::{Csr, Extension, Instruction, PrivilegeLevel, XReg, Xlen};
use crate::target::Target;

/// Most PMP entries a target may declare: pmpaddr0-15.
pub const MAX_PMP_REGIONS: usize = 16;

/// First and last HPM counter / event selector index.
const HPM: std::ops::RangeInclusive<u16> = 3..=31;

/// Whether a read of the CSR can be compared against Spike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Read {
    /// Spike and a correct core read the same value.
    Checked,
    /// The value read is implementation-dependent; the workload overwrites it.
    Clobbered,
}

/// Which values the workload may write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    Any,
    /// `frm`: 5-7 would make every FP instruction with a dynamic rounding mode
    /// illegal.
    RoundingMode,
    /// `fcsr`: only `fflags` may take arbitrary bits, `frm` as above.
    FflagsOnly,
}

/// A CSR the workload may access.
#[derive(Debug, Clone, Copy)]
pub struct WorkloadCsr {
    pub csr: Csr,
    pub read: Read,
    pub value: Value,
    /// Relative likelihood among the target's workload CSRs.
    pub weight: f64,
}

impl WorkloadCsr {
    /// Point the CSR instruction `instr` at this CSR, rewritten if needed so
    /// that whatever it writes is legal. Clearing bits keeps any legal
    /// rounding mode (0-4) legal, so register writes become `csrrc`.
    pub fn access(&self, instr: Instruction) -> Instruction {
        use Instruction::*;
        let csr = self.csr;
        match (self.value, instr) {
            (Value::Any, _) => crate::riscv::asmutil::with_csr(instr, csr),
            (_, Csrrw { rd, rs1, .. } | Csrrs { rd, rs1, .. } | Csrrc { rd, rs1, .. }) => {
                if rs1 == XReg::ZERO {
                    // A read, or a write of 0, which is a legal rounding mode.
                    crate::riscv::asmutil::with_csr(instr, csr)
                } else {
                    Csrrc { rd, rs1, csr }
                }
            }
            // The immediate is 5 bits: in fcsr it only reaches fflags.
            (Value::FflagsOnly, _) => crate::riscv::asmutil::with_csr(instr, csr),
            (Value::RoundingMode, Csrrwi { rd, uimm, .. }) => Csrrwi { rd, uimm: uimm % 5, csr },
            (Value::RoundingMode, Csrrsi { rd, uimm, .. } | Csrrci { rd, uimm, .. }) => Csrrci { rd, uimm, csr },
            (Value::RoundingMode, other) => crate::riscv::asmutil::with_csr(other, csr),
        }
    }
}

/// The CSRs the workload of `target` may access.
pub fn workload_csrs(target: &Target) -> Vec<WorkloadCsr> {
    let entry = |csr, read, value, weight| WorkloadCsr { csr, read, value, weight };
    let clobbered = |csr, weight| entry(csr, Read::Clobbered, Value::Any, weight);
    let mut csrs: Vec<_> = target
        .scratch_csrs()
        .into_iter()
        .map(|csr| entry(csr, Read::Checked, Value::Any, 4.0))
        .collect();
    if target.has(Extension::F) {
        csrs.push(entry(Csr::FFLAGS, Read::Checked, Value::Any, 2.0));
        csrs.push(entry(Csr::FRM, Read::Checked, Value::RoundingMode, 2.0));
        csrs.push(entry(Csr::FCSR, Read::Checked, Value::FflagsOnly, 2.0));
    }
    for csr in [Csr::MEPC, Csr::MCAUSE, Csr::MTVAL, Csr::MCOUNTINHIBIT, Csr::MCYCLE, Csr::MINSTRET] {
        csrs.push(clobbered(csr, 1.0));
    }
    // Each group as likely as one plain CSR, twice over.
    let hpm = HPM.count() as f64;
    for index in HPM {
        csrs.push(clobbered(hpm_counter(index), 2.0 / hpm));
        csrs.push(clobbered(hpm_event(index), 2.0 / hpm));
    }
    let lower_modes = target.has_privilege(PrivilegeLevel::User);
    if !lower_modes {
        for index in 0..target.pmp_regions {
            csrs.push(clobbered(pmp_addr(index), 2.0 / target.pmp_regions as f64));
        }
    }
    if lower_modes {
        csrs.push(clobbered(Csr::MCOUNTEREN, 1.0));
    }
    if target.has_privilege(PrivilegeLevel::Supervisor) {
        for csr in [
            Csr::MIDELEG,
            Csr::SEPC,
            Csr::SCAUSE,
            Csr::STVAL,
            Csr::SCOUNTEREN,
        ] {
            csrs.push(clobbered(csr, 1.0));
        }
    }
    csrs
}

/// Draws workload CSRs by weight.
pub struct CsrSampler {
    csrs: Vec<WorkloadCsr>,
    index: WeightedIndex<f64>,
}

impl CsrSampler {
    /// The CSRs the workload may access in `state`; `None` if there are none.
    pub fn new(target: &Target, state: &PrivState) -> Option<Self> {
        let csrs: Vec<_> = workload_csrs(target)
            .into_iter()
            .filter(|csr| state.allows_csr(csr.csr))
            .collect();
        let index = WeightedIndex::new(csrs.iter().map(|csr| csr.weight)).ok()?;
        Some(Self { csrs, index })
    }

    pub fn sample(&self, rng: &mut (impl Rng + ?Sized)) -> &WorkloadCsr {
        &self.csrs[self.index.sample(rng)]
    }
}

/// CSRs `_init` writes, in order, with their values. Only the scratch and FP
/// CSRs can be read back as written; the rest are only written, each with a
/// random value its WARL fields legalize, but `medeleg`, which gets
/// `medeleg`, and the PMP (see [`pmp`]). Every program writes a random subset
/// of the HPM counters and their event selectors.
pub fn init_csrs(
    target: &Target,
    medeleg: u64,
    window: Option<&PmpWindow>,
    rng: &mut (impl Rng + ?Sized),
) -> Vec<(Csr, u64)> {
    let xlen = target.xlen;
    let mut csrs: Vec<_> = target
        .scratch_csrs()
        .into_iter()
        .map(|csr| (csr, random_value(rng, xlen)))
        .collect();
    if target.has(Extension::F) {
        let frm = rng.random_range(0..5u64);
        csrs.push((Csr::FCSR, frm << 5 | rng.random_range(0..32u64)));
    }
    for csr in [Csr::MCOUNTINHIBIT, Csr::MCYCLE, Csr::MINSTRET, Csr::MEPC, Csr::MCAUSE, Csr::MTVAL] {
        csrs.push((csr, random_value(rng, xlen)));
    }
    let hpm_share = rng.random::<f64>();
    for index in HPM {
        if rng.random_bool(hpm_share) {
            csrs.push((hpm_event(index), event_selector(rng, xlen)));
            csrs.push((hpm_counter(index), random_value(rng, xlen)));
        }
    }
    if target.has_privilege(PrivilegeLevel::User) {
        csrs.push((Csr::MCOUNTEREN, random_value(rng, xlen)));
    }
    if target.has_privilege(PrivilegeLevel::Supervisor) {
        csrs.push((Csr::MEDELEG, medeleg));
        for csr in [Csr::MIDELEG, Csr::SEPC, Csr::SCAUSE, Csr::STVAL, Csr::SCOUNTEREN] {
            csrs.push((csr, random_value(rng, xlen)));
        }
    }
    csrs.extend(pmp(target, window, rng));
    csrs
}

/// The PMP entries' addresses, then their configurations. No entry is locked,
/// so none binds M-mode. With S or U, the lowest entries cover the window's
/// regions with its permissions, the next are random over addresses below
/// RAM, and the last covers all memory with full permissions, so S and U
/// reach everything else. Otherwise
/// entry 0 covers all memory and the others are random.
fn pmp(target: &Target, window: Option<&PmpWindow>, rng: &mut (impl Rng + ?Sized)) -> Vec<(Csr, u64)> {
    const NAPOT: u8 = 3 << 3;
    let regions = target.pmp_regions;
    let lower_modes = target.has_privilege(PrivilegeLevel::User);
    let all = if lower_modes { regions.saturating_sub(1) } else { 0 };
    let mut csrs = Vec::new();
    let mut cfgs = Vec::with_capacity(regions);
    for index in 0..regions {
        let window_region = window.and_then(|w| w.perms.get(index).map(|perms| (w.start, *perms)));
        let (addr, cfg) = if index == all {
            (u64::MAX, NAPOT | 0b111)
        } else if let Some((start, perms)) = window_region.filter(|_| lower_modes) {
            // NAPOT over 4 KiB: the base over 4, ones below the size's bit.
            let base = start + index as u64 * PMP_REGION;
            (base >> 2 | (PMP_REGION >> 3) - 1, NAPOT | perms)
        } else if lower_modes {
            below_ram(target, rng)
        } else {
            (random_value(rng, target.xlen), pmp_cfg(rng))
        };
        csrs.push((pmp_addr(index), addr));
        cfgs.push(cfg);
    }
    // RV64 packs 8 entries in each even pmpcfg, RV32 4 in each.
    let (per_csr, stride) = match target.xlen {
        Xlen::X64 => (8, 2),
        Xlen::X32 => (4, 1),
    };
    for (index, entries) in cfgs.chunks(per_csr).enumerate() {
        let value = entries.iter().rev().fold(0, |value, &cfg| value << 8 | u64::from(cfg));
        let csr = Csr::new(Csr::PMPCFG0.index() + (index * stride) as u16).expect("pmpcfg0-15");
        csrs.push((csr, value));
    }
    csrs
}

/// A PMP entry that matches no memory the program uses: any mode and
/// permissions over an address range in the lower half of the space below
/// RAM, so S and U still reach all memory through the last entry. A TOR range
/// whose base is a higher entry's is empty.
fn below_ram(target: &Target, rng: &mut (impl Rng + ?Sized)) -> (u64, u8) {
    const NAPOT: u8 = 3 << 3;
    let limit = target.physical_memory.start / 2;
    if limit < 8 {
        return (random_value(rng, target.xlen), 0);
    }
    let cfg = pmp_cfg(rng);
    let addr = rng.random_range(0..limit) >> 2;
    if cfg & NAPOT != NAPOT {
        return (addr, cfg);
    }
    // NAPOT over 2^(k + 3) bytes: k trailing ones, the range below `limit`.
    let max = (limit >> 3).ilog2();
    let k = rng.random_range(0..max);
    (addr >> 1 >> k << 1 << k | ((1 << k) - 1), cfg)
}

/// A random unlocked PMP configuration: any address-matching mode and
/// permissions but the reserved write-without-read.
pub fn pmp_cfg(rng: &mut (impl Rng + ?Sized)) -> u8 {
    let mode = rng.random_range(0..4u8) << 3;
    let mut rwx = rng.random_range(0..8u8);
    if rwx & 0b011 == 0b010 {
        rwx |= 0b001;
    }
    mode | rwx
}

/// An HPM event selector as Rocket and BOOM read it: the event set in the low
/// byte and the events to count as a mask above.
fn event_selector(rng: &mut (impl Rng + ?Sized), xlen: Xlen) -> u64 {
    let value = u64::from(rng.random::<u32>()) << 8 | rng.random_range(0..4u64);
    truncate(value, xlen)
}

fn hpm_counter(index: u16) -> Csr {
    Csr::new(Csr::MCYCLE.index() + index).expect("mhpmcounter3-31")
}

fn hpm_event(index: u16) -> Csr {
    Csr::new(Csr::MCOUNTINHIBIT.index() + index).expect("mhpmevent3-31")
}

fn pmp_addr(index: usize) -> Csr {
    Csr::new(Csr::PMPADDR0.index() + index as u16).expect("pmpaddr0-15")
}

/// A random XLEN-wide value, sign-extended to 64 bits on RV32.
pub fn random_value(rng: &mut (impl Rng + ?Sized), xlen: Xlen) -> u64 {
    truncate(rng.random::<u64>(), xlen)
}

fn truncate(value: u64, xlen: Xlen) -> u64 {
    match xlen {
        Xlen::X32 => value as u32 as i32 as i64 as u64,
        Xlen::X64 => value,
    }
}

/// Check a target's PMP entry count.
pub fn validate_pmp_regions(regions: usize) -> Result<()> {
    ensure!(
        regions <= MAX_PMP_REGIONS,
        "at most {MAX_PMP_REGIONS} PMP regions are supported, got {regions}"
    );
    Ok(())
}
