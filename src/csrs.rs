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
//!
//! The self-check compares registers against Spike, so a CSR whose value read
//! back may differ between Spike and a correct core (WARL fields,
//! implementation-defined counters, unimplemented HPM counters reading zero)
//! is [`Read::Clobbered`]: the workload may read it, but overwrites the result
//! right away.
use anyhow::{Result, ensure};
use rand::distr::{Distribution, weighted::WeightedIndex};
use rand::{Rng, RngExt};

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
    for index in 0..target.pmp_regions {
        csrs.push(clobbered(pmp_addr(index), 2.0 / target.pmp_regions as f64));
    }
    if target.has_privilege(PrivilegeLevel::User) {
        csrs.push(clobbered(Csr::MCOUNTEREN, 1.0));
    }
    if target.has_privilege(PrivilegeLevel::Supervisor) {
        for csr in [
            Csr::MEDELEG,
            Csr::MIDELEG,
            Csr::STVEC,
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
    pub fn new(target: &Target) -> Result<Self> {
        let csrs = workload_csrs(target);
        let index = WeightedIndex::new(csrs.iter().map(|csr| csr.weight))?;
        Ok(Self { csrs, index })
    }

    pub fn sample(&self, rng: &mut (impl Rng + ?Sized)) -> &WorkloadCsr {
        &self.csrs[self.index.sample(rng)]
    }
}

/// CSRs `_init` writes, in order, with their values. Only the scratch and FP
/// CSRs can be read back as written; the rest are only written, each with a
/// random value its WARL fields legalize. Every program writes a random subset
/// of the HPM counters and their event selectors.
pub fn init_csrs(target: &Target, rng: &mut (impl Rng + ?Sized)) -> Vec<(Csr, u64)> {
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
        for csr in [
            Csr::MEDELEG,
            Csr::MIDELEG,
            Csr::STVEC,
            Csr::SEPC,
            Csr::SCAUSE,
            Csr::STVAL,
            Csr::SCOUNTEREN,
        ] {
            csrs.push((csr, random_value(rng, xlen)));
        }
    }
    csrs.extend(pmp(target, rng));
    csrs
}

/// The PMP entries' addresses, then their configurations. Entry 0 covers all
/// memory with full permissions, so S and U could run; the others are random.
/// No entry is locked, so none binds M-mode.
fn pmp(target: &Target, rng: &mut (impl Rng + ?Sized)) -> Vec<(Csr, u64)> {
    const NAPOT: u8 = 3 << 3;
    let regions = target.pmp_regions;
    let mut csrs = Vec::new();
    let mut cfgs = Vec::with_capacity(regions);
    for index in 0..regions {
        let (addr, cfg) = if index == 0 {
            (u64::MAX, NAPOT | 0b111)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{MemoryRegion, Permissions};
    use crate::riscv::asmutil::csr_rd_and_addr;
    use crate::riscv::Opcode;
    use rand::{SeedableRng, rngs::StdRng};
    use std::collections::HashSet;

    fn target(xlen: Xlen, isa: &[Extension], privileges: &[PrivilegeLevel]) -> Target {
        Target::new(
            xlen,
            isa.iter().copied(),
            privileges.iter().copied(),
            HashSet::new(),
            1,
            100,
            MemoryRegion {
                start: 0x8000_0000,
                size: 0x0800_0000,
                permissions: Permissions::RWX,
            },
            None,
            8192,
            8,
        )
        .unwrap()
    }

    const S_ONLY: &[Csr] = &[
        Csr::SSCRATCH,
        Csr::MEDELEG,
        Csr::MIDELEG,
        Csr::STVEC,
        Csr::SEPC,
        Csr::SCAUSE,
        Csr::STVAL,
        Csr::SCOUNTEREN,
    ];
    /// CSRs whose writes would change what M-mode code does.
    const NEVER: &[Csr] = &[
        Csr::MSTATUS,
        Csr::MTVEC,
        Csr::MISA,
        Csr::MIE,
        Csr::MIP,
        Csr::SATP,
        Csr::SIE,
        Csr::SIP,
        Csr::SSTATUS,
        Csr::TSELECT,
        Csr::TDATA1,
    ];

    fn targets() -> Vec<(Target, bool, bool)> {
        use Extension::*;
        use PrivilegeLevel::*;
        let mut targets = Vec::new();
        for xlen in [Xlen::X32, Xlen::X64] {
            for privileges in [&[Machine][..], &[Machine, User], &[Machine, Supervisor, User]] {
                for isa in [&[I, Zicsr][..], &[I, Zicsr, F, D]] {
                    let has_s = privileges.contains(&Supervisor);
                    let has_u = privileges.contains(&User);
                    targets.push((target(xlen, isa, privileges), has_s, has_u));
                }
            }
        }
        targets
    }

    #[test]
    fn csrs_follow_the_privilege_modes_and_extensions() {
        let mut rng = StdRng::seed_from_u64(3);
        for (target, has_s, has_u) in targets() {
            let workload: Vec<_> = workload_csrs(&target).iter().map(|csr| csr.csr).collect();
            for _ in 0..20 {
                let init: Vec<_> = init_csrs(&target, &mut rng).iter().map(|(csr, _)| *csr).collect();
                for csr in workload.iter().chain(&init) {
                    assert!(!NEVER.contains(csr), "{csr} must not be written");
                    assert!(has_s || !S_ONLY.contains(csr), "{csr} needs S-mode");
                    assert!(has_u || *csr != Csr::MCOUNTEREN, "mcounteren needs U-mode");
                    let fp = [Csr::FFLAGS, Csr::FRM, Csr::FCSR].contains(csr);
                    assert!(target.has(Extension::F) || !fp, "{csr} needs F");
                }
            }
        }
    }

    #[test]
    fn only_checked_csrs_read_back_as_written() {
        for (target, _, _) in targets() {
            for csr in workload_csrs(&target) {
                let checked = target.scratch_csrs().contains(&csr.csr)
                    || [Csr::FFLAGS, Csr::FRM, Csr::FCSR].contains(&csr.csr);
                assert_eq!(csr.read == Read::Checked, checked, "{}", csr.csr);
            }
        }
    }

    #[test]
    fn pmp_entries_are_never_locked_and_entry_0_allows_everything() {
        let mut rng = StdRng::seed_from_u64(4);
        for (target, _, _) in targets() {
            for _ in 0..50 {
                let csrs = init_csrs(&target, &mut rng);
                let cfg_bytes: Vec<u8> = csrs
                    .iter()
                    .filter(|(csr, _)| (0x3a0..=0x3af).contains(&csr.index()))
                    .flat_map(|(_, value)| value.to_le_bytes().into_iter().take(target.xlen.bits() as usize / 8))
                    .collect();
                assert_eq!(cfg_bytes.len(), target.pmp_regions);
                assert_eq!(cfg_bytes[0], 0x1f);
                for cfg in cfg_bytes {
                    assert_eq!(cfg & 0x80, 0, "locked PMP entry");
                    assert_ne!(cfg & 0b011, 0b010, "reserved W without R");
                }
            }
        }
    }

    #[test]
    fn rounding_mode_writes_stay_legal() {
        let mut rng = StdRng::seed_from_u64(5);
        let (target, _, _) = targets().pop().unwrap();
        let fp: Vec<_> = workload_csrs(&target)
            .into_iter()
            .filter(|csr| csr.value != Value::Any)
            .collect();
        assert_eq!(fp.len(), 2);
        let opcodes = [Opcode::Csrrw, Opcode::Csrrs, Opcode::Csrrc, Opcode::Csrrwi, Opcode::Csrrsi, Opcode::Csrrci];
        for _ in 0..2000 {
            let csr = fp[rng.random_range(0..fp.len())];
            let op = opcodes[rng.random_range(0..opcodes.len())];
            let instr = csr.access(op.random(&mut rng, Xlen::X64).unwrap());
            assert_eq!(csr_rd_and_addr(&instr).unwrap().1, csr.csr);
            match (csr.value, instr) {
                (_, Instruction::Csrrw { rs1, .. } | Instruction::Csrrs { rs1, .. }) => assert_eq!(rs1, XReg::ZERO),
                (Value::RoundingMode, Instruction::Csrrwi { uimm, .. }) => assert!(uimm < 5),
                (Value::RoundingMode, Instruction::Csrrsi { .. }) => panic!("csrrsi can set frm to 5-7"),
                _ => {}
            }
        }
        for _ in 0..100 {
            let fcsr = init_csrs(&target, &mut rng)
                .into_iter()
                .find(|(csr, _)| *csr == Csr::FCSR)
                .unwrap()
                .1;
            assert!(fcsr >> 5 < 5, "frm {}", fcsr >> 5);
        }
    }
}
