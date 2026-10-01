use anyhow::{Ok, Result};
use rand::rngs::StdRng;
use rand::{Rng, RngExt};

use crate::basicblock::BasicBlock;
use crate::orchestrator::GlobalState;
use crate::riscv::asmutil::load_imm;
use crate::riscv::{Csr, Extension, FReg, Instruction, SAFE_CSRS, XReg, Xlen};
use crate::target::Target;
use crate::utils::cut_cake_randomly;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Privilege {
    User,
    Supervisor,
    Machine,
}
#[derive(Debug, Clone)]
pub struct HartState {
    pub privilege: Privilege,

    // mstatus state revelant to privilege transitions
    pub mpp: Privilege,
    pub spp: Privilege,

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
            privilege: Privilege::Machine,
            mpp: Privilege::Machine,
            spp: Privilege::User,

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

    pub fn run(
        &mut self,
        rng: &mut (impl Rng + ?Sized),
        target: &Target,
        state: &mut GlobalState,
    ) -> Result<()> {
        for bb in self.bbs.iter_mut() {
            bb.run(rng, target, state)?;
        }
        let tohost = state.memory.get(".tohost")?.region.start;

        // Layout of the hart's code, starting at the entry point:
        //   init: point mtvec at the handler, randomize registers
        //   workload
        //   exit: report success to tohost and spin
        //   (padding to 4 bytes, never executed)
        //   trap handler: report mcause to tohost and spin
        // mtvec is set first so a trap anywhere after it terminates the test.
        let mut init = set_mtvec(0).to_vec();
        init.extend(init_registers(rng, target)?);
        let exit = report_to_tohost(
            Instruction::Addi {
                rd: XReg::X5,
                rs1: XReg::ZERO,
                imm: 1,
            },
            tohost,
            target.xlen,
        );
        let before_handler = code_size(&init)
            + self.bbs.iter().map(|bb| code_size(&bb.instrs)).sum::<usize>()
            + code_size(&exit);
        let mut padding = Vec::new();
        if before_handler % 4 != 0 {
            // Only reachable with C enabled; never executed.
            padding.push(Instruction::cnop());
        }
        let handler_offset = before_handler + code_size(&padding);
        init.splice(..3, set_mtvec(handler_offset as i64));

        self.state.mtvec_valid = true;
        self.bbs.insert(
            0,
            BasicBlock {
                instrs: init,
                ..Default::default()
            },
        );
        self.bbs.push(BasicBlock {
            instrs: exit,
            label: Some("_exit".into()),
            ..Default::default()
        });
        for instrs in [padding, trap_handler(tohost, target.xlen)] {
            self.bbs.push(BasicBlock {
                instrs,
                ..Default::default()
            });
        }
        Ok(())
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
    for &csr in SAFE_CSRS {
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

fn code_size(instrs: &[Instruction]) -> usize {
    instrs.iter().map(Instruction::byte_len).sum()
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
