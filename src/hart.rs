use anyhow::{Ok, Result, anyhow, ensure};
use rand::rngs::StdRng;
use rand::{Rng, RngExt};

use crate::basicblock::BasicBlock;
use crate::entangle::{GOLDEN_SLOT_SIZE, PoolRef, SiteBuilder, code_size};
use crate::membase::MemBases;
use crate::orchestrator::GlobalState;
use crate::riscv::asmutil::{load_imm, load_imm_fixed, twos_complement};
use crate::riscv::{Csr, Extension, FReg, Instruction, PrivilegeLevel, XReg, Xlen};
use crate::spike::ArchState;
use crate::target::Target;
use crate::utils::cut_cake_randomly;
use crate::weights::InstrWeights;

/// Labels, exported as ELF symbols, of the parts of a hart's code.
pub const INIT_LABEL: &str = "_init";
pub const STIMULUS_LABEL: &str = "_stimulus";
pub const CHECK_LABEL: &str = "_check";
pub const EXIT_LABEL: &str = "_exit";
pub const FAIL_LABEL: &str = "_fail";
pub const TRAP_HANDLER_LABEL: &str = "_trap_handler";

/// HTIF exit code reported when the self-check finds a mismatch. It lies above
/// every `mcause + 1` the trap handler reports for exceptions, and its low byte
/// is nonzero so it survives truncation to an 8-bit process exit status.
pub const MISMATCH_EXIT_CODE: i32 = 0xaa;

/// HTIF exit code reported by `_fail`, which an entangled branch or indirect
/// jump reaches when it goes the wrong way.
pub const DIVERGENCE_EXIT_CODE: i32 = 0xab;

#[derive(Debug, Clone)]
pub struct HartState {
    pub privilege: PrivilegeLevel,

    // mstatus state revelant to privilege transitions
    pub mpp: PrivilegeLevel,
    pub spp: PrivilegeLevel,

    // whether these CSR currently contain valid continuation address
    pub mepc_valid: bool,
    pub sepc_valid: bool,

    // whether trap vectors have been initialized
    pub mtvec_valid: bool,
    pub stvec_valid: bool,

    pub medeleg: u64,
    pub mideleg: u64,

    /// Registers reserved to address memory, fixed for the whole program.
    pub mem_bases: MemBases,
}

impl Default for HartState {
    fn default() -> Self {
        Self {
            privilege: PrivilegeLevel::Machine,
            mpp: PrivilegeLevel::Machine,
            spp: PrivilegeLevel::User,

            mepc_valid: false,
            sepc_valid: false,

            mtvec_valid: false,
            stvec_valid: false,

            medeleg: 0,
            mideleg: 0,

            mem_bases: MemBases::default(),
        }
    }
}

/// `.golden` holds the sites' golden values, then, unless the constants are
/// inline, `_init`'s values and `_check`'s expected values.
#[derive(Default)]
pub struct Hart {
    pub bbs: Vec<BasicBlock>,
    state: HartState,
    /// Golden values the entanglement sites load from `.golden`.
    site_slots: usize,
    /// Whether `_init` and `_check` load their constants from `.golden`.
    pooled: bool,
    /// Values `_init` writes, see [`init_values`].
    init_values: Vec<u64>,
    /// Constants `_check` compares against, see [`check_values`].
    check_values: Vec<u64>,
}

impl Hart {
    pub fn new(num_instrs: usize, rng: &mut StdRng) -> Self {
        // Randomly distribute budgets to basic blocks
        let core = Self {
            state: HartState::default(),
            bbs: cut_cake_randomly(num_instrs, Some(1), Some(32), rng)
                .into_iter()
                .enumerate()
                .map(|(id, budget)| BasicBlock::new(id, rng.random::<bool>(), budget))
                .collect(),
            ..Default::default()
        };
        debug_assert_eq!(
            core.bbs.iter().map(|bb| bb.budget).sum::<usize>(),
            num_instrs,
        );
        core
    }

    /// Generate the hart's code, drawing each workload block's opcodes from
    /// `weights`. With `self_check`, a self-check block holding
    /// placeholder values precedes the exit; patch the reference values in with
    /// [`Hart::set_expected`] before encoding the final program. With
    /// `entangle` (holding the mid-block guard threshold), the workload holds
    /// entanglement sites; [`Hart::link`] them, then [`Hart::patch_sites_from_spike`]
    /// with Spike's register values. With `golden_section` (RV64 only), sites,
    /// `_init` and `_check` load their constants from a `.golden` section of
    /// [`Hart::golden_slots`] slots, which the caller reserves before linking.
    pub fn run(
        &mut self,
        rng: &mut (impl Rng + ?Sized),
        target: &Target,
        state: &mut GlobalState,
        weights: &InstrWeights,
        self_check: bool,
        entangle: Option<usize>,
        golden_section: bool,
    ) -> Result<()> {
        let sections: Vec<_> = state.memory.data_sections().collect();
        self.state.mem_bases = MemBases::pick(&sections, rng)?;
        let reserved: Vec<_> = self.state.mem_bases.bases.iter().map(|base| base.reg).collect();
        let mut sites =
            entangle.map(|threshold| SiteBuilder::new(target, threshold, reserved.clone(), golden_section));
        for bb in self.bbs.iter_mut() {
            bb.run(rng, target, state, &mut self.state, weights, sites.as_mut())?;
        }
        self.site_slots = sites.as_ref().map_or(0, SiteBuilder::golden_slots);
        self.pooled = golden_section;
        let tohost = state.memory.get(".tohost")?.region.start;

        // Layout of the hart's code and the symbol marking each part:
        //   _start         point mtvec at the handler (entry point)
        //   _init          randomize registers, the memory bases getting their addresses
        //   _stimulus      workload
        //   _check         (optional) compare all registers against Spike's values
        //   _exit          report the check's verdict (or success) to tohost and spin
        //   _fail          (with entanglement) report a divergence to tohost and spin
        //                  (padding to 4 bytes, never executed)
        //   _trap_handler  report mcause to tohost and spin
        // mtvec is set first so a trap anywhere after it terminates the test.
        // `_start` is exported by the ELF encoder for the whole code, so the
        // mtvec block carries no label of its own.
        self.init_values = init_values(rng, target, &self.state.mem_bases);
        let mut init = init_block(&self.init_values, target, self.init_slot())?;
        init.label = Some(INIT_LABEL.into());
        if let Some(first) = self.bbs.first_mut() {
            first.label = Some(STIMULUS_LABEL.into());
        }
        let check = if self_check {
            self.check_values = check_values(&ArchState::default(), target);
            check_block(&self.check_values, target, self.check_slot())?
        } else {
            BasicBlock::default()
        };
        let verdict = if self_check {
            // The check leaves 0 or MISMATCH_EXIT_CODE in x5.
            Instruction::Slli {
                rd: XReg::X5,
                rs1: XReg::X5,
                shamt: 1,
            }
        } else {
            Instruction::Addi {
                rd: XReg::X5,
                rs1: XReg::ZERO,
                imm: 1,
            }
        };
        let exit = report_to_tohost(verdict, tohost, target.xlen);
        let fail = if entangle.is_some() {
            report_to_tohost(
                Instruction::Addi {
                    rd: XReg::X5,
                    rs1: XReg::ZERO,
                    imm: DIVERGENCE_EXIT_CODE << 1,
                },
                tohost,
                target.xlen,
            )
        } else {
            Vec::new()
        };
        let before_handler = code_size(&set_mtvec(0))
            + code_size(&init.instrs)
            + self.bbs.iter().map(|bb| code_size(&bb.instrs)).sum::<usize>()
            + code_size(&check.instrs)
            + code_size(&exit)
            + code_size(&fail);
        let mut padding = Vec::new();
        if before_handler % 4 != 0 {
            // Only reachable with C enabled; never executed.
            padding.push(Instruction::cnop());
        }
        let handler_offset = before_handler + code_size(&padding);

        self.state.mtvec_valid = true;
        self.bbs.splice(
            0..0,
            [
                BasicBlock {
                    instrs: set_mtvec(handler_offset as i64).to_vec(),
                    ..Default::default()
                },
                init,
            ],
        );
        if self_check {
            self.bbs.push(check);
        }
        self.bbs.push(BasicBlock {
            instrs: exit,
            label: Some(EXIT_LABEL.into()),
            ..Default::default()
        });
        if entangle.is_some() {
            self.bbs.push(BasicBlock {
                instrs: fail,
                label: Some(FAIL_LABEL.into()),
                ..Default::default()
            });
        }
        self.bbs.push(BasicBlock {
            instrs: padding,
            ..Default::default()
        });
        self.bbs.push(BasicBlock {
            instrs: trap_handler(tohost, target.xlen),
            label: Some(TRAP_HANDLER_LABEL.into()),
            ..Default::default()
        });
        Ok(())
    }

    /// Byte offset of the self-check block from the hart's first instruction.
    pub fn get_self_check_code_offset(&self) -> Result<usize> {
        let index = self.get_self_check_bb_index()?;
        Ok(self.block_offset(index))
    }

    /// Number of constants loaded from `.golden`.
    pub fn golden_slots(&self) -> usize {
        self.site_slots + if self.pooled { self.init_values.len() + self.check_values.len() } else { 0 }
    }

    /// First `.golden` slot of `_init`'s values, if pooled.
    fn init_slot(&self) -> Option<usize> {
        self.pooled.then_some(self.site_slots)
    }

    /// First `.golden` slot of `_check`'s expected values, if pooled.
    fn check_slot(&self) -> Option<usize> {
        self.pooled.then_some(self.site_slots + self.init_values.len())
    }

    /// Contents of the `.golden` section, little endian: each linked site's
    /// golden value in its slot (zero before linking), then `_init`'s and
    /// `_check`'s constants if pooled. The draft holds the sites' and
    /// `_check`'s placeholders.
    pub fn golden_bytes(&self) -> Vec<u8> {
        let mut slots = vec![0; self.golden_slots()];
        for site in self.bbs.iter().flat_map(|bb| &bb.sites) {
            if let Some((slot, golden)) = site.golden_entry() {
                slots[slot] = golden;
            }
        }
        if self.pooled {
            let constants = self.init_values.iter().chain(&self.check_values);
            slots[self.site_slots..].iter_mut().zip(constants).for_each(|(slot, value)| *slot = *value);
        }
        slots.iter().flat_map(|slot| slot.to_le_bytes()).collect()
    }

    /// Fix the code's addresses, given that it starts at `text_addr` and
    /// `.golden` at `golden_addr`, and render the sites' placeholders.
    pub fn link(&mut self, text_addr: u64, golden_addr: Option<u64>, xlen: Xlen) -> Result<()> {
        let fail_addr = self
            .label_index(FAIL_LABEL)
            .map(|fail| text_addr + self.block_offset(fail) as u64);
        let mut block_addr = text_addr;
        for bb in self.bbs.iter_mut() {
            for pool_ref in &bb.pool_refs {
                pool_ref.link(&mut bb.instrs, block_addr, golden_addr)?;
            }
            for site in bb.sites.iter_mut() {
                site.link(&mut bb.instrs, block_addr, fail_addr, golden_addr)?;
            }
            bb.render_sites(xlen)?;
            block_addr += code_size(&bb.instrs) as u64;
        }
        Ok(())
    }

    /// Byte offsets, from the hart's first instruction, at which Spike must
    /// report the registers of each entanglement site, in program order. The
    /// workload is executed once, front to back, so this is also the order in
    /// which they are reached.
    pub fn probe_offsets(&self, xlen: Xlen) -> Vec<usize> {
        let mut offsets = Vec::new();
        let mut block_offset = 0;
        for bb in &self.bbs {
            for site in &bb.sites {
                if let Some(index) = site.probe_index(xlen) {
                    offsets.push(block_offset + code_size(&bb.instrs[..index]));
                }
            }
            block_offset += code_size(&bb.instrs);
        }
        offsets
    }

    /// Patch Spike's register values, one per site in [`Hart::probe_offsets`]
    /// order, into the entanglement sites. Sites keep their sizes.
    pub fn patch_entanglements(
        &mut self,
        states: &[ArchState],
        target: &Target,
        rng: &mut (impl Rng + ?Sized),
    ) -> Result<()> {
        let mut states = states.iter();
        for bb in self.bbs.iter_mut() {
            for site in bb.sites.iter_mut() {
                if site.probe_index(target.xlen).is_none() {
                    continue;
                }
                let state = states
                    .next()
                    .ok_or_else(|| anyhow!("missing register values for a site"))?;
                site.resolve(&state.xregs, target, rng)?;
            }
            bb.render_sites(target.xlen)?;
        }
        ensure!(states.next().is_none(), "more register values than sites");
        Ok(())
    }

    fn block_offset(&self, index: usize) -> usize {
        self.bbs[..index].iter().map(|bb| code_size(&bb.instrs)).sum()
    }

    fn label_index(&self, label: &str) -> Option<usize> {
        self.bbs.iter().position(|bb| bb.label.as_deref() == Some(label))
    }

    /// In the end of the program, there is a segment of self-checking code. 
    /// This func set the expected values: in `.golden` if pooled, in the code
    /// otherwise.
    pub fn set_final_expected_values(&mut self, expected: &ArchState, target: &Target) -> Result<()> {
        let index = self.get_self_check_bb_index()?;
        self.check_values = check_values(expected, target);
        if !self.pooled {
            let check = check_block(&self.check_values, target, None)?;
            ensure!(
                code_size(&check.instrs) == code_size(&self.bbs[index].instrs),
                "self-check block changed size while patching"
            );
            self.bbs[index].instrs = check.instrs;
        }
        Ok(())
    }

    fn get_self_check_bb_index(&self) -> Result<usize> {
        self.label_index(CHECK_LABEL)
            .ok_or_else(|| anyhow!("hart has no self-check block"))
    }
}

/// Values `_init` writes: the scratch CSRs, then f0-f31 (with F), then
/// x1-x31. The memory bases hold their addresses, the rest random values.
fn init_values(rng: &mut (impl Rng + ?Sized), target: &Target, bases: &MemBases) -> Vec<u64> {
    let fregs = if target.has(Extension::F) { 32 } else { 0 };
    let mut values: Vec<_> = (0..target.scratch_csrs().len() + fregs + 31)
        .map(|_| random_xlen(rng, target.xlen) as u64)
        .collect();
    let x1 = values.len() - 31;
    for base in &bases.bases {
        values[x1 + base.reg.index() as usize - 1] = base.addr;
    }
    values
}

/// `_init`: write [`init_values`] to the CSRs, FP registers and x1-x31 in
/// that order, the CSRs and FP registers through x5 when inline, so no scratch
/// value leaks into the workload. With `pool`, the values are loaded from
/// `.golden` slots from `pool` on through x31, which is loaded last. x0 is
/// hardwired to zero.
fn init_block(values: &[u64], target: &Target, pool: Option<usize>) -> Result<BasicBlock> {
    use Instruction::*;
    const PTR: XReg = XReg::X31;
    let xlen = target.xlen;
    let mut block = BasicBlock::default();
    if let Some(slot) = pool {
        let pointer = PoolRef { at: 0, rd: PTR, slot, load: false };
        block.instrs.extend(pointer.instrs(0));
        block.pool_refs.push(pointer);
    }
    let offset = |k: usize| (k as u64 * GOLDEN_SLOT_SIZE) as i32;
    // rd = values[k]
    let load = |rd: XReg, k: usize| match pool {
        Some(_) => vec![Ld { rd, rs1: PTR, imm: offset(k) }],
        None => load_imm(rd, values[k] as i64, xlen),
    };
    let mut ks = 0..values.len();
    // Scratch CSRs are plain XLEN-wide read/write registers, so any value is legal.
    for csr in target.scratch_csrs() {
        block.instrs.extend(load(XReg::X5, ks.next().expect("a CSR value")));
        block.instrs.push(Csrrw { rd: XReg::ZERO, rs1: XReg::X5, csr });
    }
    if target.has(Extension::F) {
        // Enable the FPU: set both mstatus.FS bits (Dirty) with CSRRS so the
        // other mstatus fields are preserved.
        block.instrs.push(Lui {
            rd: XReg::X5,
            imm: 0x6, // 0x6000 = 0b11 << 13 (mstatus.FS)
        });
        block.instrs.push(Csrrs { rd: XReg::ZERO, rs1: XReg::X5, csr: Csr::MSTATUS });
        // frm = RNE (round to nearest, ties to even), fflags = 0.
        block.instrs.push(Csrrw { rd: XReg::ZERO, rs1: XReg::ZERO, csr: Csr::FCSR });
        // A 64-bit pattern with D (FLD or FMV.D.X, which only exists on RV64D);
        // otherwise a random (NaN-boxed) single from the low 32 bits.
        let wide = target.has(Extension::D);
        for index in 0..32 {
            let rd = FReg::new(index)?;
            let k = ks.next().expect("an FP value");
            if pool.is_some() {
                let imm = offset(k);
                block.instrs.push(if wide { Fld { rd, rs1: PTR, imm } } else { Flw { rd, rs1: PTR, imm } });
            } else {
                block.instrs.extend(load(XReg::X5, k));
                block.instrs.push(if wide && xlen == Xlen::X64 {
                    FmvDX { rd, rs1: XReg::X5 }
                } else {
                    FmvWX { rd, rs1: XReg::X5 }
                });
            }
        }
    }
    for (index, k) in (1..32).zip(ks) {
        block.instrs.extend(load(XReg::new(index)?, k));
    }
    Ok(block)
}

/// Constants `_check` compares x1-x31, then f0-f31 (with F), against.
/// FMV.X.D moves all 64 bits but only exists on RV64D; otherwise FMV.X.W
/// moves the low 32 bits, sign-extended to XLEN.
fn check_values(expected: &ArchState, target: &Target) -> Vec<u64> {
    let xlen = target.xlen;
    let mut values: Vec<_> = expected.xregs[1..]
        .iter()
        .map(|value| twos_complement(*value, xlen) as u64)
        .collect();
    if target.has(Extension::F) {
        let wide = xlen == Xlen::X64 && target.has(Extension::D);
        values.extend(expected.fregs.iter().map(|raw| {
            if wide { *raw } else { *raw as u32 as i32 as i64 as u64 }
        }));
    }
    values
}

/// `_check`: compare every register with [`check_values`] and leave the
/// verdict in x5: 0 on a match, [`MISMATCH_EXIT_CODE`] otherwise. The
/// workload may use every register, so none is reserved: x31 is stashed in
/// mscratch to serve as the first scratch register, x1 becomes the accumulator
/// once checked, and each register checked is free to reuse afterwards. Inline
/// constants are loaded with [`load_imm_fixed`] so the block's size does not
/// depend on them. With `pool`, they are loaded from `.golden` slots from
/// `pool` on, through x2 once it is checked.
fn check_block(values: &[u64], target: &Target, pool: Option<usize>) -> Result<BasicBlock> {
    use Instruction::*;
    const ACC: XReg = XReg::X1;
    const TMP: XReg = XReg::X31;
    const PTR: XReg = XReg::X2;
    let xlen = target.xlen;

    // rd = values[k]
    let load = |block: &mut BasicBlock, rd: XReg, k: usize| match pool {
        None => block.instrs.extend(load_imm_fixed(rd, values[k] as i64, xlen)),
        // x1's and x2's are loaded before x2 is free to point at the pool.
        Some(slot) if k < 2 => {
            let direct = PoolRef { at: block.instrs.len(), rd, slot: slot + k, load: true };
            block.instrs.extend(direct.instrs(0));
            block.pool_refs.push(direct);
        }
        Some(_) => block.instrs.push(Ld {
            rd,
            rs1: PTR,
            imm: (k as u64 * GOLDEN_SLOT_SIZE) as i32,
        }),
    };
    // acc |= value ^ values[k], with `scratch` holding the constant.
    let fold = |block: &mut BasicBlock, value: XReg, scratch: XReg, k: usize| {
        load(block, scratch, k);
        block.instrs.push(Xor { rd: scratch, rs1: scratch, rs2: value });
        block.instrs.push(Or { rd: ACC, rs1: ACC, rs2: scratch });
    };

    let mut block = BasicBlock {
        label: Some(CHECK_LABEL.into()),
        ..Default::default()
    };
    block.instrs.push(Csrrw { rd: XReg::ZERO, rs1: TMP, csr: Csr::MSCRATCH });
    load(&mut block, TMP, 0);
    block.instrs.push(Xor { rd: ACC, rs1: ACC, rs2: TMP });
    fold(&mut block, XReg::X2, TMP, 1);
    if let Some(slot) = pool {
        let pointer = PoolRef { at: block.instrs.len(), rd: PTR, slot, load: false };
        block.instrs.extend(pointer.instrs(0));
        block.pool_refs.push(pointer);
    }
    for index in 3..31 {
        fold(&mut block, XReg::new(index)?, TMP, index as usize - 1);
    }
    // Restore x31 and check it with x3, which is free by now.
    block.instrs.push(Csrrw { rd: TMP, rs1: XReg::ZERO, csr: Csr::MSCRATCH });
    fold(&mut block, TMP, XReg::X3, 30);

    if target.has(Extension::F) {
        let wide = xlen == Xlen::X64 && target.has(Extension::D);
        for index in 0..32 {
            let rs1 = FReg::new(index)?;
            block.instrs.push(if wide { FmvXD { rd: TMP, rs1 } } else { FmvXW { rd: TMP, rs1 } });
            fold(&mut block, TMP, XReg::X3, 31 + index as usize);
        }
    }

    // x5 = acc != 0 ? MISMATCH_EXIT_CODE : 0, without a branch.
    block.instrs.push(Sltu { rd: XReg::X5, rs1: XReg::ZERO, rs2: ACC });
    block.instrs.push(Sub { rd: XReg::X5, rs1: XReg::ZERO, rs2: XReg::X5 });
    block.instrs.push(Andi { rd: XReg::X5, rs1: XReg::X5, imm: MISMATCH_EXIT_CODE });
    Ok(block)
}

/// Point mtvec (direct mode) at `offset` bytes from the first instruction of
/// this sequence. Always three instructions so the offset can be patched in
/// once the code in between is known.
fn set_mtvec(offset: i64) -> [Instruction; 3] {
    let hi = (offset + 0x800) >> 12;
    [
        Instruction::Auipc {
            rd: XReg::X5,
            imm: hi as i32,
        },
        Instruction::Addi {
            rd: XReg::X5,
            rs1: XReg::X5,
            imm: (offset - (hi << 12)) as i32,
        },
        Instruction::Csrrw {
            rd: XReg::ZERO,
            rs1: XReg::X5,
            csr: Csr::MTVEC,
        },
    ]
}

/// Machine-mode trap handler. Every trap reaching it (illegal instruction,
/// access fault, misaligned access, or anything else unexpected) ends the
/// test with a failing HTIF exit code of `mcause + 1`, so 0 still means pass.
fn trap_handler(tohost: u64, xlen: Xlen) -> Vec<Instruction> {
    let mut instrs = vec![
        Instruction::Csrrs {
            rd: XReg::X5,
            rs1: XReg::ZERO,
            csr: Csr::MCAUSE,
        },
        Instruction::Addi {
            rd: XReg::X5,
            rs1: XReg::X5,
            imm: 1,
        },
    ];
    instrs.extend(report_to_tohost(
        Instruction::Slli {
            rd: XReg::X5,
            rs1: XReg::X5,
            shamt: 1,
        },
        tohost,
        xlen,
    ));
    instrs
}

/// Write an HTIF exit command to `tohost` and spin until the host stops the
/// simulation. `encode` leaves `exit_code << 1` or the complete command in x5;
/// the low bit is then set, which HTIF reads as `(exit_code << 1) | 1`.
fn report_to_tohost(encode: Instruction, tohost: u64, xlen: Xlen) -> Vec<Instruction> {
    let mut instrs = vec![
        encode,
        Instruction::Ori {
            rd: XReg::X5,
            rs1: XReg::X5,
            imm: 1,
        },
    ];
    instrs.extend(load_imm(XReg::X6, tohost as i64, xlen));
    // Order all prior memory accesses before the exit command.
    instrs.push(Instruction::Fence { imm: 0x33 });
    if xlen == Xlen::X32 {
        // tohost is 64 bits wide; clear the upper word before the command.
        instrs.push(Instruction::Sw {
            rs1: XReg::X6,
            rs2: XReg::ZERO,
            imm: 4,
        });
        instrs.push(Instruction::Sw {
            rs1: XReg::X6,
            rs2: XReg::X5,
            imm: 0,
        });
    } else {
        instrs.push(Instruction::Sd {
            rs1: XReg::X6,
            rs2: XReg::X5,
            imm: 0,
        });
    }
    instrs.push(Instruction::Jal {
        rd: XReg::ZERO,
        imm: 0,
    });
    instrs
}

/// A random XLEN-wide value, sign-extended to i64 for [`load_imm`].
fn random_xlen(rng: &mut (impl Rng + ?Sized), xlen: Xlen) -> i64 {
    match xlen {
        Xlen::X32 => rng.random::<i32>().into(),
        Xlen::X64 => rng.random::<i64>(),
    }
}
