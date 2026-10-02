//! Reserved base registers for memory accesses.
//!
//! Each program reserves a few registers that hold fixed addresses inside the
//! data sections for its whole run. A memory access then reaches its target
//! through one of them instead of materializing an address per access:
//!
//! | Access | Code |
//! |---|---|
//! | 12-bit immediate (`lb`..`sd`, `flw`..`fsd`) | the access itself, rewritten to `imm(base)` |
//! | short or no immediate (compressed, `*sp`, AMO) | `addi rs1, base, off`; the access |
//! | data-dependent, a small per-program share of the above | `andi rs1, r, mask`; `add rs1, rs1, base`; the access |
//!
//! A data-dependent access adds a workload register `r`, masked to stay inside
//! the section, so its address depends on computed data without any value
//! having to be known in advance.
use anyhow::{Result, ensure};
use rand::{Rng, RngExt};

use crate::memory::Section;
use crate::riscv::registers::random_int_register;
use crate::riscv::{Instruction, InstructionClass, XReg};

/// Registers a base may be reserved from. The workload never samples them as
/// a destination (see `INT_REGISTERS` and the compressed `x8..=x15`), so only
/// generator code must keep away from them.
const BASE_CANDIDATES: [XReg; 12] = [
    XReg::GP,
    XReg::TP,
    XReg::X18,
    XReg::X19,
    XReg::X20,
    XReg::X21,
    XReg::X22,
    XReg::X23,
    XReg::X24,
    XReg::X25,
    XReg::X26,
    XReg::X27,
];
const MIN_BASES: usize = 3;
const MAX_BASES: usize = 5;

/// Upper bound of the per-program share of data-dependent accesses.
const MAX_DEP_ADDR_PROBA: f64 = 0.1;

/// Mask applied to the workload register of a data-dependent access: a
/// doubleword-aligned offset in `0..=0x7f8`, so any access width stays aligned.
const DEP_ADDR_MASK: u64 = 0x7f8;

/// A reserved register and the address it holds.
#[derive(Debug, Clone, Copy)]
pub struct MemBase {
    pub reg: XReg,
    pub addr: u64,
    /// Bounds of the section `addr` lies in.
    start: u64,
    end: u64,
}

/// The program's base registers and how its accesses use them.
#[derive(Debug, Clone, Default)]
pub struct MemBases {
    pub bases: Vec<MemBase>,
    /// Probability that an access with a 12-bit immediate is data-dependent.
    dep_proba: f64,
}

/// Code placing one memory access: `prefix` runs before `instr`.
pub struct Placement {
    pub prefix: Vec<Instruction>,
    pub instr: Instruction,
    /// Whether `prefix` mixes workload data into the address register.
    pub data_dependent: bool,
}

impl MemBases {
    /// Reserve 3 to 5 registers, each pointing at a random doubleword in one
    /// of `sections`, and draw the program's share of data-dependent accesses.
    pub fn pick(sections: &[&Section], rng: &mut (impl Rng + ?Sized)) -> Result<Self> {
        ensure!(!sections.is_empty(), "no writable section for memory accesses");
        let mut regs = BASE_CANDIDATES.to_vec();
        let count = rng.random_range(MIN_BASES..=MAX_BASES);
        let mut bases = Vec::with_capacity(count);
        for _ in 0..count {
            let reg = regs.swap_remove(rng.random_range(0..regs.len()));
            let region = &sections[rng.random_range(0..sections.len())].region;
            let (start, end) = (region.start, region.end()?);
            let first = start.next_multiple_of(8);
            ensure!(
                first + 8 <= end,
                "memory region at {start:#x} is too small for a doubleword access"
            );
            let addr = rng.random_range(first / 8..=(end - 8) / 8) * 8;
            bases.push(MemBase { reg, addr, start, end });
        }
        Ok(Self {
            bases,
            dep_proba: rng.random_range(0.0..=MAX_DEP_ADDR_PROBA),
        })
    }

    /// Point `instr` at a random aligned address reachable from one of the
    /// bases. A data-dependent access mixes in `dep`, or a random workload
    /// register without one. Returns `None` for an instruction that accesses
    /// no memory.
    pub fn place(
        &self,
        instr: Instruction,
        dep: Option<XReg>,
        rng: &mut (impl Rng + ?Sized),
    ) -> Result<Option<Placement>> {
        let Some((form, size)) = memory_operand(&instr)? else {
            return Ok(None);
        };
        ensure!(!self.bases.is_empty(), "no memory base register is reserved");
        match form {
            Form::Imm { rs1, .. } => {
                if rng.random_bool(self.dep_proba) {
                    if let Some((base, imm)) = self.pick_dep_window(size, rng) {
                        let r = dep.unwrap_or_else(|| random_int_register(rng));
                        return Ok(Some(Placement {
                            prefix: vec![
                                Instruction::Andi {
                                    rd: rs1,
                                    rs1: r,
                                    imm: DEP_ADDR_MASK as i32,
                                },
                                Instruction::Add {
                                    rd: rs1,
                                    rs1,
                                    rs2: base.reg,
                                },
                            ],
                            instr: with_address(instr, rs1, imm),
                            data_dependent: true,
                        }));
                    }
                }
                // imm(base) for an address within the immediate's reach.
                let base = self.random_base(rng);
                let lo = base.start.max(base.addr.saturating_sub(2048));
                let hi = (base.end - size).min(base.addr + 2047);
                let addr = random_aligned(lo, hi, size, rng)?;
                let imm = addr.wrapping_sub(base.addr) as i64 as i32;
                Ok(Some(Placement {
                    prefix: Vec::new(),
                    instr: with_address(instr, base.reg, imm),
                    data_dependent: false,
                }))
            }
            Form::Fixed { rs1, imm } => {
                // rs1 = base + off, where off = addr - imm - base fits ADDI.
                let base = self.random_base(rng);
                let reach_lo = (base.addr as i64 + imm - 2048).max(0) as u64;
                let lo = base.start.max(reach_lo);
                let hi = (base.end - size).min((base.addr as i64 + imm + 2047) as u64);
                let addr = random_aligned(lo, hi, size, rng)?;
                let off = addr as i64 - imm - base.addr as i64;
                Ok(Some(Placement {
                    prefix: vec![Instruction::Addi {
                        rd: rs1,
                        rs1: base.reg,
                        imm: off as i32,
                    }],
                    instr,
                    data_dependent: false,
                }))
            }
        }
    }

    fn random_base(&self, rng: &mut (impl Rng + ?Sized)) -> MemBase {
        self.bases[rng.random_range(0..self.bases.len())]
    }

    /// A base and an immediate such that `base + imm + (r & DEP_ADDR_MASK)`
    /// stays inside the base's section for every `r`, if any base allows it.
    fn pick_dep_window(&self, size: u64, rng: &mut (impl Rng + ?Sized)) -> Option<(MemBase, i32)> {
        let windows: Vec<_> = self
            .bases
            .iter()
            .filter_map(|base| {
                let lo = base.start.max(base.addr.saturating_sub(2048));
                let hi = base
                    .end
                    .checked_sub(DEP_ADDR_MASK + size)?
                    .min(base.addr + 2047);
                let first = lo.next_multiple_of(size);
                (first <= hi / size * size).then_some((*base, first, hi / size * size))
            })
            .collect();
        if windows.is_empty() {
            return None;
        }
        let (base, first, last) = windows[rng.random_range(0..windows.len())];
        let lo = rng.random_range(first / size..=last / size) * size;
        Some((base, lo.wrapping_sub(base.addr) as i64 as i32))
    }
}

/// How a memory access forms its address.
enum Form {
    /// `imm(rs1)` with a 12-bit signed immediate that can be rewritten.
    Imm { rs1: XReg, imm: i64 },
    /// The immediate is fixed (compressed forms) or absent (AMO, LR/SC), so
    /// `rs1` must be set to `addr - imm`.
    Fixed { rs1: XReg, imm: i64 },
}

/// The address form and access width of a memory access.
fn memory_operand(instr: &Instruction) -> Result<Option<(Form, u64)>> {
    use Instruction::*;
    // AMO/LR/SC have no immediate: the address is rs1 itself.
    let class = instr.class();
    if matches!(class, InstructionClass::Amo | InstructionClass::Amo64) {
        let size = if class == InstructionClass::Amo64 { 8 } else { 4 };
        let rs1 = XReg::new(((instr.encode()?.bits() >> 15) & 31) as u8)?;
        return Ok(Some((Form::Fixed { rs1, imm: 0 }, size)));
    }
    Ok(match *instr {
        Lb { rs1, imm, .. } | Lbu { rs1, imm, .. } | Sb { rs1, imm, .. } => {
            Some((Form::Imm { rs1, imm: imm.into() }, 1))
        }
        Lh { rs1, imm, .. } | Lhu { rs1, imm, .. } | Sh { rs1, imm, .. } => {
            Some((Form::Imm { rs1, imm: imm.into() }, 2))
        }
        Lw { rs1, imm, .. }
        | Lwu { rs1, imm, .. }
        | Sw { rs1, imm, .. }
        | Flw { rs1, imm, .. }
        | Fsw { rs1, imm, .. } => Some((Form::Imm { rs1, imm: imm.into() }, 4)),
        Ld { rs1, imm, .. } | Sd { rs1, imm, .. } | Fld { rs1, imm, .. } | Fsd { rs1, imm, .. } => {
            Some((Form::Imm { rs1, imm: imm.into() }, 8))
        }
        CLw { rs1, imm, .. } | CSw { rs1, imm, .. } => Some((Form::Fixed { rs1, imm: imm.into() }, 4)),
        CLd { rs1, imm, .. } | CSd { rs1, imm, .. } => Some((Form::Fixed { rs1, imm: imm.into() }, 8)),
        CLwsp { imm, .. } | CSwsp { imm, .. } => {
            Some((Form::Fixed { rs1: XReg::SP, imm: imm.into() }, 4))
        }
        CLdsp { imm, .. } | CSdsp { imm, .. } => {
            Some((Form::Fixed { rs1: XReg::SP, imm: imm.into() }, 8))
        }
        _ => None,
    })
}

/// Replace the address register and immediate of an [`Form::Imm`] access.
fn with_address(instr: Instruction, rs1: XReg, imm: i32) -> Instruction {
    use Instruction::*;
    match instr {
        Lb { rd, .. } => Lb { rd, rs1, imm },
        Lh { rd, .. } => Lh { rd, rs1, imm },
        Lw { rd, .. } => Lw { rd, rs1, imm },
        Lbu { rd, .. } => Lbu { rd, rs1, imm },
        Lhu { rd, .. } => Lhu { rd, rs1, imm },
        Lwu { rd, .. } => Lwu { rd, rs1, imm },
        Ld { rd, .. } => Ld { rd, rs1, imm },
        Sb { rs2, .. } => Sb { rs1, rs2, imm },
        Sh { rs2, .. } => Sh { rs1, rs2, imm },
        Sw { rs2, .. } => Sw { rs1, rs2, imm },
        Sd { rs2, .. } => Sd { rs1, rs2, imm },
        Flw { rd, .. } => Flw { rd, rs1, imm },
        Fsw { rs2, .. } => Fsw { rs1, rs2, imm },
        Fld { rd, .. } => Fld { rd, rs1, imm },
        Fsd { rs2, .. } => Fsd { rs1, rs2, imm },
        other => unreachable!("not an access with a 12-bit immediate: {other:?}"),
    }
}

/// A random multiple of `size` in `lo..=hi`.
fn random_aligned(lo: u64, hi: u64, size: u64, rng: &mut (impl Rng + ?Sized)) -> Result<u64> {
    let first = lo.next_multiple_of(size);
    let last = hi / size * size;
    ensure!(
        lo <= hi && first <= last,
        "no {size}-byte aligned address in {lo:#x}..={hi:#x}"
    );
    Ok(rng.random_range(first / size..=last / size) * size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{MemoryRegion, Permissions};
    use crate::riscv::{Opcode, Xlen};
    use rand::{SeedableRng, rngs::StdRng};

    fn section(start: u64, size: u64) -> Section {
        Section {
            name: "scratch".into(),
            alignment: 4,
            permissions: Permissions::RW,
            region: MemoryRegion {
                start,
                size,
                permissions: Permissions::RW,
            },
            private: false,
        }
    }

    /// Run `placement` from `regs` and return the address it accesses.
    fn address(placement: &Placement, regs: &mut [u64; 32]) -> u64 {
        for instr in &placement.prefix {
            let (rd, value) = match *instr {
                Instruction::Andi { rd, rs1, imm } => (rd, regs[rs1.index() as usize] & imm as u64),
                Instruction::Add { rd, rs1, rs2 } => (
                    rd,
                    regs[rs1.index() as usize].wrapping_add(regs[rs2.index() as usize]),
                ),
                Instruction::Addi { rd, rs1, imm } => {
                    (rd, regs[rs1.index() as usize].wrapping_add(imm as i64 as u64))
                }
                other => panic!("unexpected prefix {other:?}"),
            };
            regs[rd.index() as usize] = value;
        }
        let (Form::Imm { rs1, imm } | Form::Fixed { rs1, imm }, _) =
            memory_operand(&placement.instr).unwrap().unwrap();
        regs[rs1.index() as usize].wrapping_add(imm as u64)
    }

    #[test]
    fn every_access_lands_aligned_inside_its_section() {
        let mut rng = StdRng::seed_from_u64(1);
        let sections = [section(0x8000_0000, 8192), section(0x8001_0000, 4096)];
        let refs: Vec<_> = sections.iter().collect();
        let opcodes: Vec<_> = Opcode::ALL
            .iter()
            .copied()
            .filter(|op| op.supports_xlen(Xlen::X64))
            .filter(|op| {
                op.random(&mut rng, Xlen::X64)
                    .is_ok_and(|instr| memory_operand(&instr).unwrap().is_some())
            })
            .collect();
        let mut dependent = 0;
        for _ in 0..200 {
            let mut bases = MemBases::pick(&refs, &mut rng).unwrap();
            bases.dep_proba = 0.5;
            for _ in 0..200 {
                let op = opcodes[rng.random_range(0..opcodes.len())];
                let instr = op.random(&mut rng, Xlen::X64).unwrap();
                let (_, size) = memory_operand(&instr).unwrap().unwrap();
                let placement = bases.place(instr, None, &mut rng).unwrap().unwrap();
                placement.instr.encode_for(Xlen::X64).unwrap();
                for prefix in &placement.prefix {
                    prefix.encode_for(Xlen::X64).unwrap();
                    let rd = written_rd(prefix);
                    assert!(!bases.bases.iter().any(|base| base.reg == rd));
                }
                dependent += placement.data_dependent as usize;
                // Data-dependent accesses are checked at both ends of the mask.
                for fill in [0, u64::MAX] {
                    let mut regs = [fill; 32];
                    regs[0] = 0;
                    for base in &bases.bases {
                        regs[base.reg.index() as usize] = base.addr;
                    }
                    let addr = address(&placement, &mut regs);
                    assert_eq!(addr % size, 0, "{:?} at {addr:#x}", placement.instr);
                    assert!(
                        sections.iter().any(|s| {
                            addr >= s.region.start && addr + size <= s.region.end().unwrap()
                        }),
                        "{:?} at {addr:#x} is outside every section",
                        placement.instr
                    );
                }
            }
        }
        assert!(dependent > 0);
    }

    fn written_rd(instr: &Instruction) -> XReg {
        match *instr {
            Instruction::Andi { rd, .. } | Instruction::Add { rd, .. } | Instruction::Addi { rd, .. } => rd,
            other => panic!("unexpected prefix {other:?}"),
        }
    }

    #[test]
    fn bases_are_distinct_and_unused_by_the_workload() {
        let mut rng = StdRng::seed_from_u64(2);
        let sections = [section(0x8000_0000, 8192)];
        let refs: Vec<_> = sections.iter().collect();
        for _ in 0..100 {
            let bases = MemBases::pick(&refs, &mut rng).unwrap();
            assert!((MIN_BASES..=MAX_BASES).contains(&bases.bases.len()));
            for (i, base) in bases.bases.iter().enumerate() {
                assert!(!crate::riscv::registers::INT_REGISTERS.contains(&base.reg));
                assert!(!(8..=15).contains(&base.reg.index()));
                assert!(bases.bases[..i].iter().all(|other| other.reg != base.reg));
            }
        }
    }
}
