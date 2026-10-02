use anyhow::{Ok, Result, anyhow, ensure};
use rand::rngs::StdRng;
use rand::{Rng, RngExt};

use crate::basicblock::BasicBlock;
use crate::entangle::{SiteBuilder, code_size};
use crate::orchestrator::GlobalState;
use crate::riscv::asmutil::{load_imm, load_imm_fixed, twos_complement};
use crate::riscv::{Csr, Extension, FReg, Instruction, PrivilegeLevel, XReg, Xlen};
use crate::spike::ArchState;
use crate::target::Target;
use crate::utils::cut_cake_randomly;

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
        }
    }
}

#[derive(Default)]
pub struct Hart {
    pub bbs: Vec<BasicBlock>,
    state: HartState,
}

impl Hart {
    pub fn new(num_instrs: usize, rng: &mut StdRng) -> Self {
        // Randomly distribute budgets to basic blocks
        let core = Self {
            state: HartState::default(),
            bbs: cut_cake_randomly(num_instrs, Some(1), Some(32), rng)
                .into_iter()
                .enumerate()
                .map(|(id, budget)| BasicBlock::new(id, rand::random::<bool>(), budget))
                .collect(),
        };
        debug_assert_eq!(
            core.bbs.iter().map(|bb| bb.budget).sum::<usize>(),
            num_instrs,
        );
        core
    }

    /// Generate the hart's code. With `self_check`, a self-check block holding
    /// placeholder values precedes the exit; patch the reference values in with
    /// [`Hart::set_expected`] before encoding the final program. With
    /// `entangle` (holding the mid-block guard threshold), the workload holds
    /// entanglement sites; [`Hart::link`] them, then [`Hart::resolve_sites`]
    /// with Spike's register values.
    pub fn run(
        &mut self,
        rng: &mut (impl Rng + ?Sized),
        target: &Target,
        state: &mut GlobalState,
        self_check: bool,
        entangle: Option<usize>,
    ) -> Result<()> {
        let mut sites = entangle.map(|threshold| SiteBuilder::new(target, threshold));
        for bb in self.bbs.iter_mut() {
            bb.run(rng, target, state, sites.as_mut())?;
        }
        let tohost = state.memory.get(".tohost")?.region.start;

        // Layout of the hart's code and the symbol marking each part:
        //   _start         point mtvec at the handler (entry point)
        //   _init          randomize registers
        //   _stimulus      workload
        //   _check         (optional) compare all registers against Spike's values
        //   _exit          report the check's verdict (or success) to tohost and spin
        //   _fail          (with entanglement) report a divergence to tohost and spin
        //                  (padding to 4 bytes, never executed)
        //   _trap_handler  report mcause to tohost and spin
        // mtvec is set first so a trap anywhere after it terminates the test.
        // `_start` is exported by the ELF encoder for the whole code, so the
        // mtvec block carries no label of its own.
        let init = init_registers(rng, target)?;
        if let Some(first) = self.bbs.first_mut() {
            first.label = Some(STIMULUS_LABEL.into());
        }
        let check = if self_check {
            self_check_block(&ArchState::default(), target)?
        } else {
            Vec::new()
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
            + code_size(&init)
            + self.bbs.iter().map(|bb| code_size(&bb.instrs)).sum::<usize>()
            + code_size(&check)
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
                BasicBlock {
                    instrs: init,
                    label: Some(INIT_LABEL.into()),
                    ..Default::default()
                },
            ],
        );
        if self_check {
            self.bbs.push(BasicBlock {
                instrs: check,
                label: Some(CHECK_LABEL.into()),
                ..Default::default()
            });
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
    pub fn check_offset(&self) -> Result<usize> {
        let index = self.check_index()?;
        Ok(self.block_offset(index))
    }

    /// Fix the entanglement sites' code addresses, given that the hart's code
    /// starts at `text_addr`, and render their placeholders.
    pub fn link(&mut self, text_addr: u64, xlen: Xlen) -> Result<()> {
        let Some(fail) = self.label_index(FAIL_LABEL) else {
            return Ok(());
        };
        let fail_addr = text_addr + self.block_offset(fail) as u64;
        let mut block_addr = text_addr;
        for bb in self.bbs.iter_mut() {
            for site in bb.sites.iter_mut() {
                site.link(&mut bb.instrs, block_addr, fail_addr)?;
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
    pub fn resolve_sites(
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

    /// Replace the self-check's placeholder values with `expected`. The block
    /// keeps its size, so no other code moves and the register state reaching
    /// it is unchanged.
    pub fn set_expected(&mut self, expected: &ArchState, target: &Target) -> Result<()> {
        let index = self.check_index()?;
        let instrs = self_check_block(expected, target)?;
        ensure!(
            code_size(&instrs) == code_size(&self.bbs[index].instrs),
            "self-check block changed size while patching"
        );
        self.bbs[index].instrs = instrs;
        Ok(())
    }

    fn check_index(&self) -> Result<usize> {
        self.label_index(CHECK_LABEL)
            .ok_or_else(|| anyhow!("hart has no self-check block"))
    }

    // pub fn encode(&self, target: &Target) -> Result<Vec<u8>> {
    //     let mut bytes = Vec::new();
    //     for bb in &self.bbs {
    //         bytes.extend_from_slice(&bb.encode(target)?);
    //     }
    //     Ok(bytes)
    // }
}

/// Randomize the architectural register state. The CSRs the workload may
/// access and the FP registers are initialized first because they go through
/// x5 as scratch; x1-x31 are written last so no scratch value leaks into the
/// workload. x0 is hardwired to zero.
fn init_registers(rng: &mut (impl Rng + ?Sized), target: &Target) -> Result<Vec<Instruction>> {
    let xlen = target.xlen;
    let mut instrs = Vec::new();
    // Scratch CSRs are plain XLEN-wide read/write registers, so any value is legal.
    for csr in target.scratch_csrs() {
        instrs.extend(load_imm(XReg::X5, random_xlen(rng, xlen), xlen));
        instrs.push(Instruction::Csrrw {
            rd: XReg::ZERO,
            rs1: XReg::X5,
            csr,
        });
    }
    if target.has(Extension::F) {
        // Enable the FPU: set both mstatus.FS bits (Dirty) with CSRRS so the
        // other mstatus fields are preserved.
        instrs.push(Instruction::Lui {
            rd: XReg::X5,
            imm: 0x6, // 0x6000 = 0b11 << 13 (mstatus.FS)
        });
        instrs.push(Instruction::Csrrs {
            rd: XReg::ZERO,
            rs1: XReg::X5,
            csr: Csr::MSTATUS,
        });
        // frm = RNE (round to nearest, ties to even), fflags = 0.
        instrs.push(Instruction::Csrrw {
            rd: XReg::ZERO,
            rs1: XReg::ZERO,
            csr: Csr::FCSR,
        });
        // FMV.D.X moves a full 64-bit pattern but only exists on RV64D;
        // otherwise FMV.W.X writes a random (NaN-boxed) single.
        let wide = xlen == Xlen::X64 && target.has(Extension::D);
        for index in 0..32 {
            let rd = FReg::new(index)?;
            instrs.extend(load_imm(XReg::X5, random_xlen(rng, xlen), xlen));
            instrs.push(if wide {
                Instruction::FmvDX { rd, rs1: XReg::X5 }
            } else {
                Instruction::FmvWX { rd, rs1: XReg::X5 }
            });
        }
    }
    for index in 1..32 {
        instrs.extend(load_imm(XReg::new(index)?, random_xlen(rng, xlen), xlen));
    }
    Ok(instrs)
}

/// Compare every register with `expected` and leave the verdict in x5: 0 on a
/// match, [`MISMATCH_EXIT_CODE`] otherwise. The workload may use every
/// register, so none is reserved: x31 is stashed in mscratch to serve as the
/// first scratch register, x1 becomes the accumulator once checked, and each
/// register checked is free to reuse afterwards. Constants are loaded with
/// [`load_imm_fixed`] so the block's size does not depend on `expected`.
fn self_check_block(expected: &ArchState, target: &Target) -> Result<Vec<Instruction>> {
    use Instruction::*;
    const ACC: XReg = XReg::X1;
    const TMP: XReg = XReg::X31;
    let xlen = target.xlen;
    let xval = |index: usize| twos_complement(expected.xregs[index], xlen);

    // acc |= value ^ constant, with `scratch` holding the constant.
    let fold = |value: XReg, scratch: XReg, constant: i64| {
        let mut seq = load_imm_fixed(scratch, constant, xlen);
        seq.push(Xor {
            rd: scratch,
            rs1: scratch,
            rs2: value,
        });
        seq.push(Or {
            rd: ACC,
            rs1: ACC,
            rs2: scratch,
        });
        seq
    };

    let mut instrs = vec![Csrrw {
        rd: XReg::ZERO,
        rs1: TMP,
        csr: Csr::MSCRATCH,
    }];
    instrs.extend(load_imm_fixed(TMP, xval(1), xlen));
    instrs.push(Xor {
        rd: ACC,
        rs1: ACC,
        rs2: TMP,
    });
    for index in 2..31 {
        instrs.extend(fold(XReg::new(index as u8)?, TMP, xval(index)));
    }
    // Restore x31 and check it with x2, which is free by now.
    instrs.push(Csrrw {
        rd: TMP,
        rs1: XReg::ZERO,
        csr: Csr::MSCRATCH,
    });
    instrs.extend(fold(TMP, XReg::X2, xval(31)));

    if target.has(Extension::F) {
        // FMV.X.D moves all 64 bits but only exists on RV64D; otherwise
        // FMV.X.W moves the low 32 bits, sign-extended to XLEN.
        let wide = xlen == Xlen::X64 && target.has(Extension::D);
        for index in 0..32 {
            let rs1 = FReg::new(index as u8)?;
            instrs.push(if wide {
                FmvXD { rd: TMP, rs1 }
            } else {
                FmvXW { rd: TMP, rs1 }
            });
            let raw = expected.fregs[index];
            let constant = if wide { raw as i64 } else { raw as u32 as i32 as i64 };
            instrs.extend(fold(TMP, XReg::X2, constant));
        }
    }

    // x5 = acc != 0 ? MISMATCH_EXIT_CODE : 0, without a branch.
    instrs.push(Sltu {
        rd: XReg::X5,
        rs1: XReg::ZERO,
        rs2: ACC,
    });
    instrs.push(Sub {
        rd: XReg::X5,
        rs1: XReg::ZERO,
        rs2: XReg::X5,
    });
    instrs.push(Andi {
        rd: XReg::X5,
        rs1: XReg::X5,
        imm: MISMATCH_EXIT_CODE,
    });
    Ok(instrs)
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
