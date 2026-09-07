//! Operand generation and high-level action lowering. Encoding stays in `riscv`.

use rand::{rngs::StdRng, seq::IteratorRandom, seq::SliceRandom, Rng};

use crate::{
    riscv::{asmutil::li_into_reg, instruction::OperandSource, FloatReg, IntReg, ROUNDING_MODES},
    Instruction, InstructionClass, InstructionKind, Result,
};

use super::{GenerationAction, GenerationChoice, GeneratorParams};

/// Constraints available to instruction and action lowering. This is not a
/// simulator or a platform memory map; generated memory/CSR accesses may trap.
#[derive(Debug, Clone, Copy)]
pub struct GenerationContext {
    pub is_64bit: bool,
    pub authorize_privileges: bool,
}

impl Default for GenerationContext {
    fn default() -> Self {
        Self {
            is_64bit: true,
            authorize_privileges: true,
        }
    }
}

impl From<&GeneratorParams> for GenerationContext {
    fn from(params: &GeneratorParams) -> Self {
        Self {
            is_64bit: params.is_64bit,
            authorize_privileges: params.authorize_privileges,
        }
    }
}

// Use caller-saved registers, leaving sp/gp/tp and callee-saved registers alone.
// Destinations are always nonzero, so generated dependencies are not discarded.
const INT_REGS: &[IntReg] = &[
    IntReg::t0,
    IntReg::t1,
    IntReg::t2,
    IntReg::t3,
    IntReg::t4,
    IntReg::t5,
    IntReg::t6,
    IntReg::a0,
    IntReg::a1,
    IntReg::a2,
    IntReg::a3,
    IntReg::a4,
    IntReg::a5,
    IntReg::a6,
    IntReg::a7,
];
const FLOAT_REGS: &[FloatReg] = &[
    FloatReg::ft0,
    FloatReg::ft1,
    FloatReg::ft2,
    FloatReg::ft3,
    FloatReg::ft4,
    FloatReg::ft5,
    FloatReg::ft6,
    FloatReg::ft7,
    FloatReg::ft8,
    FloatReg::ft9,
    FloatReg::ft10,
    FloatReg::ft11,
    FloatReg::fa0,
    FloatReg::fa1,
    FloatReg::fa2,
    FloatReg::fa3,
    FloatReg::fa4,
    FloatReg::fa5,
    FloatReg::fa6,
    FloatReg::fa7,
];

struct RandomOperands<'a> {
    prng: &'a mut StdRng,
    kind: InstructionKind,
    context: &'a GenerationContext,
}

impl OperandSource for RandomOperands<'_> {
    fn int_reg(&mut self) -> IntReg {
        *INT_REGS.choose(self.prng).unwrap()
    }

    fn float_reg(&mut self) -> FloatReg {
        *FLOAT_REGS.choose(self.prng).unwrap()
    }

    fn imm(&mut self) -> i32 {
        use InstructionClass as C;
        use InstructionKind as K;
        match self.kind {
            K::Lui | K::Auipc => self.prng.gen_range(0..=0xfffff),
            K::Fence => self.prng.gen_range(0..=0xff),
            K::FenceI => 0,
            // Fall through regardless of the branch condition. General CFG
            // construction and relocation of indirect targets are future work.
            K::Jal => 4,
            K::Jalr => self.prng.gen_range(-512..=511) * 4,
            _ => match self.kind.class() {
                Some(C::Branch) => 4,
                Some(C::Memory | C::Memory64 | C::FloatMemory | C::DoubleMemory) => {
                    self.prng.gen_range(-256..=255) * 8
                }
                _ => self.prng.gen_range(-2048..=2047),
            },
        }
    }

    fn shamt(&mut self) -> i32 {
        let width = if self.context.is_64bit && self.kind.class() == Some(InstructionClass::Alu) {
            64
        } else {
            32
        };
        self.prng.gen_range(0..width)
    }

    fn rm(&mut self) -> i32 {
        *ROUNDING_MODES.choose(self.prng).unwrap()
    }

    fn csr(&mut self) -> i32 {
        self.prng.gen_range(0..=0xfff)
    }

    fn uimm(&mut self) -> i32 {
        self.prng.gen_range(0..32)
    }

    fn ordering(&mut self) -> bool {
        self.prng.gen()
    }
}

impl InstructionClass {
    fn validate(self, context: &GenerationContext) -> Result<()> {
        if self.requires_rv64() && !context.is_64bit {
            return Err(format!("instruction class {self:?} requires RV64").into());
        }
        if self == Self::Csr && !context.authorize_privileges {
            return Err("CSR generation requires authorize_privileges".into());
        }
        Ok(())
    }

    /// Select an opcode from the shared ISA table and supply its typed operands.
    /// Compressed opcodes are not sampled: legacy compressed helper constraints
    /// remain the responsibility of explicit `Instruction` callers.
    pub fn lower(self, prng: &mut StdRng, context: &GenerationContext) -> Result<Instruction> {
        self.validate(context)?;
        let kind = self
            .kinds()
            .choose(prng)
            .ok_or("instruction class has no opcodes")?;
        Ok(kind.with_operands(&mut RandomOperands {
            prng,
            kind,
            context,
        }))
    }
}

impl GenerationAction {
    fn validate(self) -> Result<()> {
        match self {
            Self::RegFsm | Self::Exception | Self::FreePolluted | Self::CreateAddressDependency => {
                Ok(())
            }
            _ => Err(format!(
                "action {self:?} requires platform/state support; lowering is not implemented"
            )
            .into()),
        }
    }

    /// Emit a complete sequence. Platform-dependent actions are explicit errors
    /// until their required context and lowering are implemented.
    pub fn lower(self, prng: &mut StdRng, context: &GenerationContext) -> Result<Vec<Instruction>> {
        self.validate()?;
        match self {
            Self::RegFsm => {
                let rd = *INT_REGS.choose(prng).unwrap();
                let value: u32 = prng.gen();
                let (upper, lower) = li_into_reg(u64::from(value), false);
                Ok(vec![
                    Instruction::Lui {
                        rd,
                        imm: upper as i32,
                    },
                    if context.is_64bit {
                        Instruction::Addiw {
                            rd,
                            rs1: rd,
                            imm: lower,
                        }
                    } else {
                        Instruction::Addi {
                            rd,
                            rs1: rd,
                            imm: lower,
                        }
                    },
                ])
            }
            Self::Exception => Ok(vec![if prng.gen() {
                Instruction::Ecall
            } else {
                Instruction::Ebreak
            }]),
            Self::FreePolluted => Ok(Vec::new()),
            Self::CreateAddressDependency => {
                let source = *INT_REGS.choose(prng).unwrap();
                let address = *INT_REGS
                    .iter()
                    .filter(|&&reg| reg != source)
                    .choose(prng)
                    .unwrap();
                let rd = *INT_REGS.choose(prng).unwrap();
                Ok(vec![
                    Instruction::Addi {
                        rd: address,
                        rs1: source,
                        imm: 0,
                    },
                    Instruction::Lw {
                        rd,
                        rs1: address,
                        imm: 0,
                    },
                ])
            }
            _ => unreachable!("unsupported actions rejected by validate"),
        }
    }
}

impl GenerationChoice {
    pub(super) fn validate(self, context: &GenerationContext) -> Result<()> {
        match self {
            Self::Instruction(class) => class.validate(context),
            Self::Action(action) => action.validate(),
        }
    }

    /// Lower one policy choice to zero, one, or multiple concrete instructions.
    pub fn lower(self, prng: &mut StdRng, context: &GenerationContext) -> Result<Vec<Instruction>> {
        match self {
            Self::Instruction(class) => Ok(vec![class.lower(prng, context)?]),
            Self::Action(action) => action.lower(prng, context),
        }
    }
}
