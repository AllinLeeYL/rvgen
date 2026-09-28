use crate::riscv::asmutil::csr_rd_and_addr;
use crate::riscv::asmutil::load_imm32;
use crate::riscv::{Instruction, InstructionClass, Opcode, XReg};
use crate::runtime::{Runtime, SCRATCH_BASE_REGISTER, ScratchWindow, Target};
use anyhow::{Result, ensure};
use rand::{Rng, RngExt};

#[derive(Default)]
pub struct BasicBlock {
    pub id: usize,
    pub is_smc: bool,
    pub budget: usize,
    pub instrs: Vec<Instruction>,
}

impl BasicBlock {
    pub fn new(id: usize, is_smc: bool, budget: usize) -> Self {
        Self {
            id: id,
            is_smc: is_smc,
            budget: budget,
            instrs: Vec::with_capacity(budget),
        }
    }

    pub fn run(&mut self, rng: &mut (impl Rng + ?Sized), runtime: &Runtime) -> Result<()> {
        let candidates: Vec<_> = runtime
            .workload_opcodes()
            .filter(|opcode| {
                !matches!(
                    opcode.class(),
                    InstructionClass::Branch | InstructionClass::Jal | InstructionClass::Jalr
                ) && !matches!(
                    opcode,
                    Opcode::Ecall | Opcode::Ebreak | Opcode::CEbreak | Opcode::Sret | Opcode::Mret
                )
            })
            .collect();
        ensure!(
            self.budget == 0 || !candidates.is_empty(),
            "no enabled instructions remain for a straight-line workload"
        );
        let xlen = runtime.target().xlen();
        let window = runtime.scratch_window();
        for _ in 0..self.budget {
            let opcode = candidates[rng.random_range(0..candidates.len())];
            let mut instr = opcode.random(rng, xlen)?;
            if let Some(setup) = sandbox_memory(&mut instr, window, rng)? {
                self.instrs.push(setup);
            }
            self.instrs.push(instr);

            // If current instr reads an implementation-dependent CSR into rd,
            // overwrite rd with a random value so the test stays deterministic.
            if let Some((rd, csr)) = csr_rd_and_addr(&instr) {
                if rd != XReg::ZERO && csr.is_implementation_dependent() {
                    let fixup = load_imm32(rd, rng.random::<i32>(), xlen);
                    self.instrs.extend_from_slice(&fixup);
                }
            }
        }
        Ok(())
    }

    pub fn encode(&self, target: &Target) -> Result<Vec<u8>> {
        let mut bytes = Vec::with_capacity(self.instrs.len() * 4);
        for instr in &self.instrs {
            target.emit(*instr, &mut bytes)?;
        }
        Ok(bytes)
    }
}

/// Keep each memory access within the runtime's allocated scratch section.
/// gp is reserved by the runtime and never selected by the operand sampler.
fn sandbox_memory(
    instr: &mut Instruction,
    window: ScratchWindow,
    rng: &mut (impl Rng + ?Sized),
) -> Result<Option<Instruction>> {
    use Instruction::*;
    let mut pick = |size: i32, lo: i32, hi: i32| -> Result<i32> {
        let (lo, hi) = window.offset_bounds(size, lo, hi)?;
        Ok(rng.random_range(lo / size..=hi / size) * size)
    };
    let mv = |rd| Addi {
        rd,
        rs1: SCRATCH_BASE_REGISTER,
        imm: 0,
    };

    // AMO/LR/SC have no immediate: the address is rs1 itself.
    let class = instr.class();
    if matches!(class, InstructionClass::Amo | InstructionClass::Amo64) {
        let size = if class == InstructionClass::Amo64 {
            8
        } else {
            4
        };
        let rs1 = XReg::new(((instr.encode()?.bits() >> 15) & 31) as u8)?;
        return Ok(Some(Addi {
            rd: rs1,
            rs1: SCRATCH_BASE_REGISTER,
            imm: pick(size, -2048, 2047)?,
        }));
    }

    Ok(match instr {
        Lb { rs1, imm, .. } | Lbu { rs1, imm, .. } | Sb { rs1, imm, .. } => {
            *imm = pick(1, -2048, 2047)?;
            Some(mv(*rs1))
        }
        Lh { rs1, imm, .. } | Lhu { rs1, imm, .. } | Sh { rs1, imm, .. } => {
            *imm = pick(2, -2048, 2047)?;
            Some(mv(*rs1))
        }
        Lw { rs1, imm, .. }
        | Lwu { rs1, imm, .. }
        | Sw { rs1, imm, .. }
        | Flw { rs1, imm, .. }
        | Fsw { rs1, imm, .. } => {
            *imm = pick(4, -2048, 2047)?;
            Some(mv(*rs1))
        }
        Ld { rs1, imm, .. } | Sd { rs1, imm, .. } | Fld { rs1, imm, .. } | Fsd { rs1, imm, .. } => {
            *imm = pick(8, -2048, 2047)?;
            Some(mv(*rs1))
        }
        CLw { rs1, imm, .. } | CSw { rs1, imm, .. } => {
            *imm = pick(4, 0, 124)?;
            Some(mv(*rs1))
        }
        CLd { rs1, imm, .. } | CSd { rs1, imm, .. } => {
            *imm = pick(8, 0, 248)?;
            Some(mv(*rs1))
        }
        CLwsp { imm, .. } | CSwsp { imm, .. } => {
            *imm = pick(4, 0, 252)?;
            Some(mv(XReg::SP))
        }
        CLdsp { imm, .. } | CSdsp { imm, .. } => {
            *imm = pick(8, 0, 504)?;
            Some(mv(XReg::SP))
        }
        _ => None,
    })
}
