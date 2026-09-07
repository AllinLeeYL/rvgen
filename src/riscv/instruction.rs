//! Operand-bearing instructions, separate from the low-level bit encoders.
//!
//! Each enum variant stores only its instruction's operands. Encoding is computed
//! on demand, so mutating operands cannot leave stale cached machine code. Integer
//! and floating-point registers have distinct types. Fuzzer-specific information
//! (hart, event ID, observed address, coverage) belongs outside this ISA layer.
//!
//! The existing encoders remain the single source of bit layouts. Their immediate
//! masking, assertions, and known compressed-helper limitations still apply; this
//! is not an ISA/XLEN validator. Use raw variants for deliberate illegal encodings.

use std::fmt;

use super::{encoding, rvwmo, FloatReg, IntReg};

/// Instruction families for weighted selection, separate from generator actions.
/// The `64` families require RV64; F/D/A/M families require those extensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InstructionClass {
    Alu,
    Alu64,
    MulDiv,
    MulDiv64,
    Memory,
    Memory64,
    Branch,
    Jal,
    Jalr,
    Amo,
    Amo64,
    FloatMemory,
    Float,
    Float64,
    DoubleMemory,
    Double,
    Double64,
    Fence,
    Csr,
}

impl InstructionClass {
    pub const fn requires_rv64(self) -> bool {
        matches!(
            self,
            Self::Alu64
                | Self::MulDiv64
                | Self::Memory64
                | Self::Amo64
                | Self::Float64
                | Self::Double64
        )
    }

    /// Stable declaration order from the instruction table.
    pub fn kinds(self) -> impl Iterator<Item = InstructionKind> {
        InstructionKind::ALL
            .iter()
            .copied()
            .filter(move |kind| kind.class() == Some(self))
    }
}

// The table requests only the operands present in a variant. Operand policy
// implements this interface in the generator; the ISA layer has no RNG or state.
pub(crate) trait OperandSource {
    fn int_reg(&mut self) -> IntReg;
    fn float_reg(&mut self) -> FloatReg;
    fn imm(&mut self) -> i32;
    fn shamt(&mut self) -> i32;
    fn rm(&mut self) -> i32;
    fn csr(&mut self) -> i32;
    fn uimm(&mut self) -> i32;
    fn ordering(&mut self) -> bool;
}

macro_rules! operand {
    ($source:ident, $field:ident, IntReg) => {
        $source.int_reg()
    };
    ($source:ident, $field:ident, FloatReg) => {
        $source.float_reg()
    };
    ($source:ident, $field:ident, i32) => {
        $source.$field()
    };
    ($source:ident, $field:ident, bool) => {
        $source.ordering()
    };
}

macro_rules! instruction_class {
    () => {
        None
    };
    ($class:ident) => {
        Some(InstructionClass::$class)
    };
}

/// Encoded bytes with an explicit instruction width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EncodedInstruction {
    Standard(u32),
    Compressed(u16),
}

impl EncodedInstruction {
    fn compressed(word: u32) -> Self {
        Self::Compressed(u16::try_from(word).expect("compressed encoding exceeds 16 bits"))
    }

    pub const fn byte_len(self) -> usize {
        match self {
            Self::Standard(_) => 4,
            Self::Compressed(_) => 2,
        }
    }

    pub const fn bits(self) -> u32 {
        match self {
            Self::Standard(word) => word,
            Self::Compressed(halfword) => halfword as u32,
        }
    }

    pub fn append_bytes(self, bytes: &mut Vec<u8>) {
        match self {
            Self::Standard(word) => bytes.extend_from_slice(&word.to_le_bytes()),
            Self::Compressed(halfword) => bytes.extend_from_slice(&halfword.to_le_bytes()),
        }
    }
}

impl fmt::Display for EncodedInstruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Standard(word) => write!(f, ".4byte 0x{word:08x}"),
            Self::Compressed(halfword) => write!(f, ".2byte 0x{halfword:04x}"),
        }
    }
}

// One declaration ties operands, encoder, width, mnemonic, and class together. Adding an
// instruction here adds its variant and all dispatch methods without string-based
// opcode selection or a separately maintained encoding match.
macro_rules! instructions {
    ($(
        $variant:ident $( { $( $field:ident : $ty:ident ),+ $(,)? } )?
        => $encoder:ident ( $( $arg:expr ),* ), $width:literal, $mnemonic:literal $(, $class:ident)?;
    )*) => {
        /// Operand-free ISA opcode identity, derived from the instruction table.
        /// Raw encodings intentionally have no kind.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum InstructionKind {
            $( $variant, )*
        }

        impl InstructionKind {
            pub const ALL: &'static [Self] = &[$( Self::$variant, )*];

            /// `None` means this opcode is only constructed explicitly or by an
            /// action, not by the current instruction-family sampler.
            pub const fn class(self) -> Option<InstructionClass> {
                match self {
                    $( Self::$variant => instruction_class!($($class)?), )*
                }
            }

            pub(crate) fn with_operands(self, source: &mut impl OperandSource) -> Instruction {
                match self {
                    $( Self::$variant => Instruction::$variant $( {
                        $( $field: operand!(source, $field, $ty) ),+
                    } )?, )*
                }
            }
        }

        /// An instruction with editable, named operands.
        ///
        /// Register numbers are represented by [`IntReg`] or [`FloatReg`].
        /// Immediate fields follow the corresponding low-level encoder's units:
        /// branch/jump offsets are bytes, and LUI/AUIPC use the upper 20-bit field.
        /// `Standard` and `Compressed` retain the raw-word API for custom opcodes,
        /// illegal-instruction tests, and callers that already have encoded bytes.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Instruction {
            $( $variant $( { $( $field: $ty ),+ } )?, )*
            Standard(u32),
            Compressed(u16),
        }

        impl Instruction {
            pub const fn kind(&self) -> Option<InstructionKind> {
                match self {
                    $( Self::$variant $( { $( $field: _ ),+ } )? => Some(InstructionKind::$variant), )*
                    Self::Standard(_) | Self::Compressed(_) => None,
                }
            }

            pub const fn class(&self) -> Option<InstructionClass> {
                match self.kind() {
                    Some(kind) => kind.class(),
                    None => None,
                }
            }

            /// Encode current operands; no machine code is cached.
            ///
            /// # Panics
            /// Panics when a low-level encoder assertion fails, or a compressed
            /// encoder returns bits outside a halfword. No silent truncation occurs.
            pub fn encode(&self) -> EncodedInstruction {
                match *self {
                    $( Self::$variant $( { $( $field ),+ } )? => {
                        let word = encoding::$encoder($( $arg ),*);
                        if $width == 2 {
                            EncodedInstruction::compressed(word)
                        } else {
                            EncodedInstruction::Standard(word)
                        }
                    }, )*
                    Self::Standard(word) => EncodedInstruction::Standard(word),
                    Self::Compressed(halfword) => EncodedInstruction::Compressed(halfword),
                }
            }

            /// Width is available before encoding, for layout and offset updates.
            pub const fn byte_len(&self) -> usize {
                match self {
                    $( Self::$variant $( { $( $field: _ ),+ } )? => $width, )*
                    Self::Standard(_) => 4,
                    Self::Compressed(_) => 2,
                }
            }

            /// Canonical mnemonic; raw words intentionally have no semantic metadata.
            pub const fn mnemonic(&self) -> Option<&'static str> {
                match self {
                    $( Self::$variant $( { $( $field: _ ),+ } )? => Some($mnemonic), )*
                    Self::Standard(_) | Self::Compressed(_) => None,
                }
            }
        }
    };
}

instructions! {
    // rv32i
    Lui { rd: IntReg, imm: i32 } => rv32i_lui(rd as i32, imm), 4, "lui", Alu;
    Auipc { rd: IntReg, imm: i32 } => rv32i_auipc(rd as i32, imm), 4, "auipc", Alu;
    Jal { rd: IntReg, imm: i32 } => rv32i_jal(rd as i32, imm), 4, "jal", Jal;
    Jalr { rd: IntReg, rs1: IntReg, imm: i32 } => rv32i_jalr(rd as i32, rs1 as i32, imm), 4, "jalr", Jalr;
    Beq { rs1: IntReg, rs2: IntReg, imm: i32 } => rv32i_beq(rs1 as i32, rs2 as i32, imm), 4, "beq", Branch;
    Bne { rs1: IntReg, rs2: IntReg, imm: i32 } => rv32i_bne(rs1 as i32, rs2 as i32, imm), 4, "bne", Branch;
    Blt { rs1: IntReg, rs2: IntReg, imm: i32 } => rv32i_blt(rs1 as i32, rs2 as i32, imm), 4, "blt", Branch;
    Bge { rs1: IntReg, rs2: IntReg, imm: i32 } => rv32i_bge(rs1 as i32, rs2 as i32, imm), 4, "bge", Branch;
    Bltu { rs1: IntReg, rs2: IntReg, imm: i32 } => rv32i_bltu(rs1 as i32, rs2 as i32, imm), 4, "bltu", Branch;
    Bgeu { rs1: IntReg, rs2: IntReg, imm: i32 } => rv32i_bgeu(rs1 as i32, rs2 as i32, imm), 4, "bgeu", Branch;
    Lb { rd: IntReg, rs1: IntReg, imm: i32 } => rv32i_lb(rd as i32, rs1 as i32, imm), 4, "lb", Memory;
    Lh { rd: IntReg, rs1: IntReg, imm: i32 } => rv32i_lh(rd as i32, rs1 as i32, imm), 4, "lh", Memory;
    Lw { rd: IntReg, rs1: IntReg, imm: i32 } => rv32i_lw(rd as i32, rs1 as i32, imm), 4, "lw", Memory;
    Lbu { rd: IntReg, rs1: IntReg, imm: i32 } => rv32i_lbu(rd as i32, rs1 as i32, imm), 4, "lbu", Memory;
    Lhu { rd: IntReg, rs1: IntReg, imm: i32 } => rv32i_lhu(rd as i32, rs1 as i32, imm), 4, "lhu", Memory;
    Sb { rs1: IntReg, rs2: IntReg, imm: i32 } => rv32i_sb(rs1 as i32, rs2 as i32, imm), 4, "sb", Memory;
    Sh { rs1: IntReg, rs2: IntReg, imm: i32 } => rv32i_sh(rs1 as i32, rs2 as i32, imm), 4, "sh", Memory;
    Sw { rs1: IntReg, rs2: IntReg, imm: i32 } => rv32i_sw(rs1 as i32, rs2 as i32, imm), 4, "sw", Memory;
    Addi { rd: IntReg, rs1: IntReg, imm: i32 } => rv32i_addi(rd as i32, rs1 as i32, imm), 4, "addi", Alu;
    Slti { rd: IntReg, rs1: IntReg, imm: i32 } => rv32i_slti(rd as i32, rs1 as i32, imm), 4, "slti", Alu;
    Sltiu { rd: IntReg, rs1: IntReg, imm: i32 } => rv32i_sltiu(rd as i32, rs1 as i32, imm), 4, "sltiu", Alu;
    Xori { rd: IntReg, rs1: IntReg, imm: i32 } => rv32i_xori(rd as i32, rs1 as i32, imm), 4, "xori", Alu;
    Ori { rd: IntReg, rs1: IntReg, imm: i32 } => rv32i_ori(rd as i32, rs1 as i32, imm), 4, "ori", Alu;
    Andi { rd: IntReg, rs1: IntReg, imm: i32 } => rv32i_andi(rd as i32, rs1 as i32, imm), 4, "andi", Alu;
    Slli { rd: IntReg, rs1: IntReg, shamt: i32 } => rv32i_slli(rd as i32, rs1 as i32, shamt), 4, "slli", Alu;
    Srli { rd: IntReg, rs1: IntReg, shamt: i32 } => rv32i_srli(rd as i32, rs1 as i32, shamt), 4, "srli", Alu;
    Srai { rd: IntReg, rs1: IntReg, shamt: i32 } => rv32i_srai(rd as i32, rs1 as i32, shamt), 4, "srai", Alu;
    Add { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32i_add(rd as i32, rs1 as i32, rs2 as i32), 4, "add", Alu;
    Sub { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32i_sub(rd as i32, rs1 as i32, rs2 as i32), 4, "sub", Alu;
    Sll { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32i_sll(rd as i32, rs1 as i32, rs2 as i32), 4, "sll", Alu;
    Slt { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32i_slt(rd as i32, rs1 as i32, rs2 as i32), 4, "slt", Alu;
    Sltu { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32i_sltu(rd as i32, rs1 as i32, rs2 as i32), 4, "sltu", Alu;
    Xor { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32i_xor(rd as i32, rs1 as i32, rs2 as i32), 4, "xor", Alu;
    Srl { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32i_srl(rd as i32, rs1 as i32, rs2 as i32), 4, "srl", Alu;
    Sra { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32i_sra(rd as i32, rs1 as i32, rs2 as i32), 4, "sra", Alu;
    Or { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32i_or(rd as i32, rs1 as i32, rs2 as i32), 4, "or", Alu;
    And { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32i_and(rd as i32, rs1 as i32, rs2 as i32), 4, "and", Alu;
    Fence { imm: i32 } => rv32i_fence(imm), 4, "fence", Fence;
    Ecall => rv32i_ecall(), 4, "ecall";
    Ebreak => rv32i_ebreak(), 4, "ebreak";
    // rv64i
    Lwu { rd: IntReg, rs1: IntReg, imm: i32 } => rv64i_lwu(rd as i32, rs1 as i32, imm), 4, "lwu", Memory64;
    Ld { rd: IntReg, rs1: IntReg, imm: i32 } => rv64i_ld(rd as i32, rs1 as i32, imm), 4, "ld", Memory64;
    Sd { rs1: IntReg, rs2: IntReg, imm: i32 } => rv64i_sd(rs1 as i32, rs2 as i32, imm), 4, "sd", Memory64;
    Addiw { rd: IntReg, rs1: IntReg, imm: i32 } => rv64i_addiw(rd as i32, rs1 as i32, imm), 4, "addiw", Alu64;
    Slliw { rd: IntReg, rs1: IntReg, shamt: i32 } => rv64i_slliw(rd as i32, rs1 as i32, shamt), 4, "slliw", Alu64;
    Srliw { rd: IntReg, rs1: IntReg, shamt: i32 } => rv64i_srliw(rd as i32, rs1 as i32, shamt), 4, "srliw", Alu64;
    Sraiw { rd: IntReg, rs1: IntReg, shamt: i32 } => rv64i_sraiw(rd as i32, rs1 as i32, shamt), 4, "sraiw", Alu64;
    Addw { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv64i_addw(rd as i32, rs1 as i32, rs2 as i32), 4, "addw", Alu64;
    Subw { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv64i_subw(rd as i32, rs1 as i32, rs2 as i32), 4, "subw", Alu64;
    Sllw { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv64i_sllw(rd as i32, rs1 as i32, rs2 as i32), 4, "sllw", Alu64;
    Srlw { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv64i_srlw(rd as i32, rs1 as i32, rs2 as i32), 4, "srlw", Alu64;
    Sraw { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv64i_sraw(rd as i32, rs1 as i32, rs2 as i32), 4, "sraw", Alu64;
    // rv32m
    Mul { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32m_mul(rd as i32, rs1 as i32, rs2 as i32), 4, "mul", MulDiv;
    Mulh { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32m_mulh(rd as i32, rs1 as i32, rs2 as i32), 4, "mulh", MulDiv;
    Mulhsu { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32m_mulhsu(rd as i32, rs1 as i32, rs2 as i32), 4, "mulhsu", MulDiv;
    Mulhu { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32m_mulhu(rd as i32, rs1 as i32, rs2 as i32), 4, "mulhu", MulDiv;
    Div { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32m_div(rd as i32, rs1 as i32, rs2 as i32), 4, "div", MulDiv;
    Divu { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32m_divu(rd as i32, rs1 as i32, rs2 as i32), 4, "divu", MulDiv;
    Rem { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32m_rem(rd as i32, rs1 as i32, rs2 as i32), 4, "rem", MulDiv;
    Remu { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32m_remu(rd as i32, rs1 as i32, rs2 as i32), 4, "remu", MulDiv;
    // rv64m
    Mulw { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv64m_mulw(rd as i32, rs1 as i32, rs2 as i32), 4, "mulw", MulDiv64;
    Divw { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv64m_divw(rd as i32, rs1 as i32, rs2 as i32), 4, "divw", MulDiv64;
    Divuw { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv64m_divuw(rd as i32, rs1 as i32, rs2 as i32), 4, "divuw", MulDiv64;
    Remw { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv64m_remw(rd as i32, rs1 as i32, rs2 as i32), 4, "remw", MulDiv64;
    Remuw { rd: IntReg, rs1: IntReg, rs2: IntReg } => rv64m_remuw(rd as i32, rs1 as i32, rs2 as i32), 4, "remuw", MulDiv64;
    // rv32a
    LrW { aq: bool, rl: bool, rd: IntReg, rs1: IntReg } => rv32a_lrw(aq, rl, rd as i32, rs1 as i32, 0), 4, "lr.w", Amo;
    ScW { aq: bool, rl: bool, rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32a_scw(aq, rl, rd as i32, rs1 as i32, rs2 as i32), 4, "sc.w", Amo;
    AmoswapW { aq: bool, rl: bool, rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32a_amoswapw(aq, rl, rd as i32, rs1 as i32, rs2 as i32), 4, "amoswap.w", Amo;
    AmoaddW { aq: bool, rl: bool, rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32a_amoaddw(aq, rl, rd as i32, rs1 as i32, rs2 as i32), 4, "amoadd.w", Amo;
    AmoandW { aq: bool, rl: bool, rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32a_amoandw(aq, rl, rd as i32, rs1 as i32, rs2 as i32), 4, "amoand.w", Amo;
    AmoorW { aq: bool, rl: bool, rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32a_amoorw(aq, rl, rd as i32, rs1 as i32, rs2 as i32), 4, "amoor.w", Amo;
    AmoxorW { aq: bool, rl: bool, rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32a_amoxorw(aq, rl, rd as i32, rs1 as i32, rs2 as i32), 4, "amoxor.w", Amo;
    AmomaxW { aq: bool, rl: bool, rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32a_amomaxw(aq, rl, rd as i32, rs1 as i32, rs2 as i32), 4, "amomax.w", Amo;
    AmomaxuW { aq: bool, rl: bool, rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32a_amomaxuw(aq, rl, rd as i32, rs1 as i32, rs2 as i32), 4, "amomaxu.w", Amo;
    AmominW { aq: bool, rl: bool, rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32a_amominw(aq, rl, rd as i32, rs1 as i32, rs2 as i32), 4, "amomin.w", Amo;
    AmominuW { aq: bool, rl: bool, rd: IntReg, rs1: IntReg, rs2: IntReg } => rv32a_amominuw(aq, rl, rd as i32, rs1 as i32, rs2 as i32), 4, "amominu.w", Amo;
    // rv64a
    LrD { aq: bool, rl: bool, rd: IntReg, rs1: IntReg } => rv64a_lrd(aq, rl, rd as i32, rs1 as i32, 0), 4, "lr.d", Amo64;
    ScD { aq: bool, rl: bool, rd: IntReg, rs1: IntReg, rs2: IntReg } => rv64a_scd(aq, rl, rd as i32, rs1 as i32, rs2 as i32), 4, "sc.d", Amo64;
    AmoswapD { aq: bool, rl: bool, rd: IntReg, rs1: IntReg, rs2: IntReg } => rv64a_amoswapd(aq, rl, rd as i32, rs1 as i32, rs2 as i32), 4, "amoswap.d", Amo64;
    AmoaddD { aq: bool, rl: bool, rd: IntReg, rs1: IntReg, rs2: IntReg } => rv64a_amoaddd(aq, rl, rd as i32, rs1 as i32, rs2 as i32), 4, "amoadd.d", Amo64;
    AmoandD { aq: bool, rl: bool, rd: IntReg, rs1: IntReg, rs2: IntReg } => rv64a_amoandd(aq, rl, rd as i32, rs1 as i32, rs2 as i32), 4, "amoand.d", Amo64;
    AmoorD { aq: bool, rl: bool, rd: IntReg, rs1: IntReg, rs2: IntReg } => rv64a_amoord(aq, rl, rd as i32, rs1 as i32, rs2 as i32), 4, "amoor.d", Amo64;
    AmoxorD { aq: bool, rl: bool, rd: IntReg, rs1: IntReg, rs2: IntReg } => rv64a_amoxord(aq, rl, rd as i32, rs1 as i32, rs2 as i32), 4, "amoxor.d", Amo64;
    AmomaxD { aq: bool, rl: bool, rd: IntReg, rs1: IntReg, rs2: IntReg } => rv64a_amomaxd(aq, rl, rd as i32, rs1 as i32, rs2 as i32), 4, "amomax.d", Amo64;
    AmomaxuD { aq: bool, rl: bool, rd: IntReg, rs1: IntReg, rs2: IntReg } => rv64a_amomaxud(aq, rl, rd as i32, rs1 as i32, rs2 as i32), 4, "amomaxu.d", Amo64;
    AmominD { aq: bool, rl: bool, rd: IntReg, rs1: IntReg, rs2: IntReg } => rv64a_amomind(aq, rl, rd as i32, rs1 as i32, rs2 as i32), 4, "amomin.d", Amo64;
    AmominuD { aq: bool, rl: bool, rd: IntReg, rs1: IntReg, rs2: IntReg } => rv64a_amominud(aq, rl, rd as i32, rs1 as i32, rs2 as i32), 4, "amominu.d", Amo64;
    // rv32f
    Flw { rd: FloatReg, rs1: IntReg, imm: i32 } => rv32f_flw(rd as i32, rs1 as i32, imm), 4, "flw", FloatMemory;
    Fsw { rs1: IntReg, rs2: FloatReg, imm: i32 } => rv32f_fsw(rs1 as i32, rs2 as i32, imm), 4, "fsw", FloatMemory;
    FmaddS { rd: FloatReg, rs1: FloatReg, rs2: FloatReg, rs3: FloatReg, rm: i32 } => rv32f_fmadds(rd as i32, rs1 as i32, rs2 as i32, rs3 as i32, rm), 4, "fmadd.s", Float;
    FmsubS { rd: FloatReg, rs1: FloatReg, rs2: FloatReg, rs3: FloatReg, rm: i32 } => rv32f_fmsubs(rd as i32, rs1 as i32, rs2 as i32, rs3 as i32, rm), 4, "fmsub.s", Float;
    FnmsubS { rd: FloatReg, rs1: FloatReg, rs2: FloatReg, rs3: FloatReg, rm: i32 } => rv32f_fnmsubs(rd as i32, rs1 as i32, rs2 as i32, rs3 as i32, rm), 4, "fnmsub.s", Float;
    FnmaddS { rd: FloatReg, rs1: FloatReg, rs2: FloatReg, rs3: FloatReg, rm: i32 } => rv32f_fnmadds(rd as i32, rs1 as i32, rs2 as i32, rs3 as i32, rm), 4, "fnmadd.s", Float;
    FaddS { rd: FloatReg, rs1: FloatReg, rs2: FloatReg, rm: i32 } => rv32f_fadds(rd as i32, rs1 as i32, rs2 as i32, rm), 4, "fadd.s", Float;
    FsubS { rd: FloatReg, rs1: FloatReg, rs2: FloatReg, rm: i32 } => rv32f_fsubs(rd as i32, rs1 as i32, rs2 as i32, rm), 4, "fsub.s", Float;
    FmulS { rd: FloatReg, rs1: FloatReg, rs2: FloatReg, rm: i32 } => rv32f_fmuls(rd as i32, rs1 as i32, rs2 as i32, rm), 4, "fmul.s", Float;
    FdivS { rd: FloatReg, rs1: FloatReg, rs2: FloatReg, rm: i32 } => rv32f_fdivs(rd as i32, rs1 as i32, rs2 as i32, rm), 4, "fdiv.s", Float;
    FsqrtS { rd: FloatReg, rs1: FloatReg, rm: i32 } => rv32f_fsqrts(rd as i32, rs1 as i32, rm), 4, "fsqrt.s", Float;
    FsgnjS { rd: FloatReg, rs1: FloatReg, rs2: FloatReg } => rv32f_fsgnjs(rd as i32, rs1 as i32, rs2 as i32), 4, "fsgnj.s", Float;
    FsgnjnS { rd: FloatReg, rs1: FloatReg, rs2: FloatReg } => rv32f_fsgnjns(rd as i32, rs1 as i32, rs2 as i32), 4, "fsgnjn.s", Float;
    FsgnjxS { rd: FloatReg, rs1: FloatReg, rs2: FloatReg } => rv32f_fsgnjxs(rd as i32, rs1 as i32, rs2 as i32), 4, "fsgnjx.s", Float;
    FminS { rd: FloatReg, rs1: FloatReg, rs2: FloatReg } => rv32f_fmins(rd as i32, rs1 as i32, rs2 as i32), 4, "fmin.s", Float;
    FmaxS { rd: FloatReg, rs1: FloatReg, rs2: FloatReg } => rv32f_fmaxs(rd as i32, rs1 as i32, rs2 as i32), 4, "fmax.s", Float;
    FcvtWS { rd: IntReg, rs1: FloatReg, rm: i32 } => rv32f_fcvtws(rd as i32, rs1 as i32, rm), 4, "fcvt.w.s", Float;
    FcvtWuS { rd: IntReg, rs1: FloatReg, rm: i32 } => rv32f_fcvtwus(rd as i32, rs1 as i32, rm), 4, "fcvt.wu.s", Float;
    FmvXW { rd: IntReg, rs1: FloatReg } => rv32f_fmvxw(rd as i32, rs1 as i32), 4, "fmv.x.w", Float;
    FeqS { rd: IntReg, rs1: FloatReg, rs2: FloatReg } => rv32f_feqs(rd as i32, rs1 as i32, rs2 as i32), 4, "feq.s", Float;
    FltS { rd: IntReg, rs1: FloatReg, rs2: FloatReg } => rv32f_flts(rd as i32, rs1 as i32, rs2 as i32), 4, "flt.s", Float;
    FleS { rd: IntReg, rs1: FloatReg, rs2: FloatReg } => rv32f_fles(rd as i32, rs1 as i32, rs2 as i32), 4, "fle.s", Float;
    FclassS { rd: IntReg, rs1: FloatReg } => rv32f_fclasss(rd as i32, rs1 as i32), 4, "fclass.s", Float;
    FcvtSW { rd: FloatReg, rs1: IntReg, rm: i32 } => rv32f_fcvtsw(rd as i32, rs1 as i32, rm), 4, "fcvt.s.w", Float;
    FcvtSWu { rd: FloatReg, rs1: IntReg, rm: i32 } => rv32f_fcvtswu(rd as i32, rs1 as i32, rm), 4, "fcvt.s.wu", Float;
    FmvWX { rd: FloatReg, rs1: IntReg } => rv32f_fmvwx(rd as i32, rs1 as i32), 4, "fmv.w.x", Float;
    // rv64f
    FcvtLS { rd: IntReg, rs1: FloatReg, rm: i32 } => rv64f_fcvtls(rd as i32, rs1 as i32, rm), 4, "fcvt.l.s", Float64;
    FcvtLuS { rd: IntReg, rs1: FloatReg, rm: i32 } => rv64f_fcvtlus(rd as i32, rs1 as i32, rm), 4, "fcvt.lu.s", Float64;
    FcvtSL { rd: FloatReg, rs1: IntReg, rm: i32 } => rv64f_fcvtsl(rd as i32, rs1 as i32, rm), 4, "fcvt.s.l", Float64;
    FcvtSLu { rd: FloatReg, rs1: IntReg, rm: i32 } => rv64f_fcvtslu(rd as i32, rs1 as i32, rm), 4, "fcvt.s.lu", Float64;
    // rv32d
    Fld { rd: FloatReg, rs1: IntReg, imm: i32 } => rv32d_fld(rd as i32, rs1 as i32, imm), 4, "fld", DoubleMemory;
    Fsd { rs1: IntReg, rs2: FloatReg, imm: i32 } => rv32d_fsd(rs1 as i32, rs2 as i32, imm), 4, "fsd", DoubleMemory;
    FmaddD { rd: FloatReg, rs1: FloatReg, rs2: FloatReg, rs3: FloatReg, rm: i32 } => rv32d_fmaddd(rd as i32, rs1 as i32, rs2 as i32, rs3 as i32, rm), 4, "fmadd.d", Double;
    FmsubD { rd: FloatReg, rs1: FloatReg, rs2: FloatReg, rs3: FloatReg, rm: i32 } => rv32d_fmsubd(rd as i32, rs1 as i32, rs2 as i32, rs3 as i32, rm), 4, "fmsub.d", Double;
    FnmsubD { rd: FloatReg, rs1: FloatReg, rs2: FloatReg, rs3: FloatReg, rm: i32 } => rv32d_fnmsubd(rd as i32, rs1 as i32, rs2 as i32, rs3 as i32, rm), 4, "fnmsub.d", Double;
    FnmaddD { rd: FloatReg, rs1: FloatReg, rs2: FloatReg, rs3: FloatReg, rm: i32 } => rv32d_fnmaddd(rd as i32, rs1 as i32, rs2 as i32, rs3 as i32, rm), 4, "fnmadd.d", Double;
    FaddD { rd: FloatReg, rs1: FloatReg, rs2: FloatReg, rm: i32 } => rv32d_faddd(rd as i32, rs1 as i32, rs2 as i32, rm), 4, "fadd.d", Double;
    FsubD { rd: FloatReg, rs1: FloatReg, rs2: FloatReg, rm: i32 } => rv32d_fsubd(rd as i32, rs1 as i32, rs2 as i32, rm), 4, "fsub.d", Double;
    FmulD { rd: FloatReg, rs1: FloatReg, rs2: FloatReg, rm: i32 } => rv32d_fmuld(rd as i32, rs1 as i32, rs2 as i32, rm), 4, "fmul.d", Double;
    FdivD { rd: FloatReg, rs1: FloatReg, rs2: FloatReg, rm: i32 } => rv32d_fdivd(rd as i32, rs1 as i32, rs2 as i32, rm), 4, "fdiv.d", Double;
    FsqrtD { rd: FloatReg, rs1: FloatReg, rm: i32 } => rv32d_fsqrtd(rd as i32, rs1 as i32, rm), 4, "fsqrt.d", Double;
    FsgnjD { rd: FloatReg, rs1: FloatReg, rs2: FloatReg } => rv32d_fsgnjd(rd as i32, rs1 as i32, rs2 as i32), 4, "fsgnj.d", Double;
    FsgnjnD { rd: FloatReg, rs1: FloatReg, rs2: FloatReg } => rv32d_fsgnjnd(rd as i32, rs1 as i32, rs2 as i32), 4, "fsgnjn.d", Double;
    FsgnjxD { rd: FloatReg, rs1: FloatReg, rs2: FloatReg } => rv32d_fsgnjxd(rd as i32, rs1 as i32, rs2 as i32), 4, "fsgnjx.d", Double;
    FminD { rd: FloatReg, rs1: FloatReg, rs2: FloatReg } => rv32d_fmind(rd as i32, rs1 as i32, rs2 as i32), 4, "fmin.d", Double;
    FmaxD { rd: FloatReg, rs1: FloatReg, rs2: FloatReg } => rv32d_fmaxd(rd as i32, rs1 as i32, rs2 as i32), 4, "fmax.d", Double;
    FcvtSD { rd: FloatReg, rs1: FloatReg, rm: i32 } => rv32d_fcvtsd(rd as i32, rs1 as i32, rm), 4, "fcvt.s.d", Double;
    FcvtDS { rd: FloatReg, rs1: FloatReg, rm: i32 } => rv32d_fcvtds(rd as i32, rs1 as i32, rm), 4, "fcvt.d.s", Double;
    FeqD { rd: IntReg, rs1: FloatReg, rs2: FloatReg } => rv32d_feqd(rd as i32, rs1 as i32, rs2 as i32), 4, "feq.d", Double;
    FltD { rd: IntReg, rs1: FloatReg, rs2: FloatReg } => rv32d_fltd(rd as i32, rs1 as i32, rs2 as i32), 4, "flt.d", Double;
    FleD { rd: IntReg, rs1: FloatReg, rs2: FloatReg } => rv32d_fled(rd as i32, rs1 as i32, rs2 as i32), 4, "fle.d", Double;
    FclassD { rd: IntReg, rs1: FloatReg } => rv32d_fclassd(rd as i32, rs1 as i32), 4, "fclass.d", Double;
    FcvtWD { rd: IntReg, rs1: FloatReg, rm: i32 } => rv32d_fcvtwd(rd as i32, rs1 as i32, rm), 4, "fcvt.w.d", Double;
    FcvtWuD { rd: IntReg, rs1: FloatReg, rm: i32 } => rv32d_fcvtwud(rd as i32, rs1 as i32, rm), 4, "fcvt.wu.d", Double;
    FcvtDW { rd: FloatReg, rs1: IntReg, rm: i32 } => rv32d_fcvtdw(rd as i32, rs1 as i32, rm), 4, "fcvt.d.w", Double;
    FcvtDWu { rd: FloatReg, rs1: IntReg, rm: i32 } => rv32d_fcvtdwu(rd as i32, rs1 as i32, rm), 4, "fcvt.d.wu", Double;
    // rv64d
    FcvtLD { rd: IntReg, rs1: FloatReg, rm: i32 } => rv64d_fcvtld(rd as i32, rs1 as i32, rm), 4, "fcvt.l.d", Double64;
    FcvtLuD { rd: IntReg, rs1: FloatReg, rm: i32 } => rv64d_fcvtlud(rd as i32, rs1 as i32, rm), 4, "fcvt.lu.d", Double64;
    FmvXD { rd: IntReg, rs1: FloatReg } => rv64d_fmvxd(rd as i32, rs1 as i32), 4, "fmv.x.d", Double64;
    FcvtDL { rd: FloatReg, rs1: IntReg, rm: i32 } => rv64d_fcvtdl(rd as i32, rs1 as i32, rm), 4, "fcvt.d.l", Double64;
    FcvtDLu { rd: FloatReg, rs1: IntReg, rm: i32 } => rv64d_fcvtdlu(rd as i32, rs1 as i32, rm), 4, "fcvt.d.lu", Double64;
    FmvDX { rd: FloatReg, rs1: IntReg } => rv64d_fmvdx(rd as i32, rs1 as i32), 4, "fmv.d.x", Double64;
    // rv32c
    CMv { rd: IntReg, rs2: IntReg } => rv32ic_mv(rd as i32, rs2 as i32), 2, "c.mv";
    CAdd { rd: IntReg, rs2: IntReg } => rv32ic_add(rd as i32, rs2 as i32), 2, "c.add";
    CAnd { rd: IntReg, rs2: IntReg } => rv32ic_and(rd as i32, rs2 as i32), 2, "c.and";
    COr { rd: IntReg, rs2: IntReg } => rv32ic_or(rd as i32, rs2 as i32), 2, "c.or";
    CXor { rd: IntReg, rs2: IntReg } => rv32ic_xor(rd as i32, rs2 as i32), 2, "c.xor";
    CSub { rd: IntReg, rs2: IntReg } => rv32ic_sub(rd as i32, rs2 as i32), 2, "c.sub";
    CLui { rd: IntReg, imm: i32 } => rv32ic_lui(rd as i32, imm), 2, "c.lui";
    CAddi16sp { rd: IntReg, imm: i32 } => rv32ic_addi16sp(rd as i32, imm), 2, "c.addi16sp";
    CAddi4spn { rd: IntReg, imm: i32 } => rv32ic_addi4spn(rd as i32, imm), 2, "c.addi4spn";
    CAddi { rd: IntReg, imm: i32 } => rv32ic_addi(rd as i32, imm), 2, "c.addi";
    CLi { rd: IntReg, imm: i32 } => rv32ic_li(rd as i32, imm), 2, "c.li";
    CSlli { rd: IntReg, imm: i32 } => rv32ic_slli(rd as i32, imm), 2, "c.slli";
    CAndi { rs1: IntReg, imm: i32 } => rv32ic_andi(rs1 as i32, imm), 2, "c.andi";
    CSrli { rs1: IntReg, imm: i32 } => rv32ic_srli(rs1 as i32, imm), 2, "c.srli";
    CSrai { rs1: IntReg, imm: i32 } => rv32ic_srai(rs1 as i32, imm), 2, "c.srai";
    CBeqz { rs1: IntReg, imm: i32 } => rv32ic_beqz(rs1 as i32, imm), 2, "c.beqz";
    CBnez { rs1: IntReg, imm: i32 } => rv32ic_bnez(rs1 as i32, imm), 2, "c.bnez";
    CJal { imm: i32 } => rv32ic_jal(imm), 2, "c.jal";
    CJ { imm: i32 } => rv32ic_j(imm), 2, "c.j";
    CJr { rs1: IntReg } => rv32ic_jr(rs1 as i32), 2, "c.jr";
    CJalr { rs1: IntReg } => rv32ic_jalr(rs1 as i32), 2, "c.jalr";
    CEbreak => rv32ic_ebreak(), 2, "c.ebreak";
    CLwsp { rd: IntReg, imm: i32 } => rv32ic_lwsp(rd as i32, imm), 2, "c.lwsp";
    CLw { rd: IntReg, rs1: IntReg, imm: i32 } => rv32ic_lw(rd as i32, rs1 as i32, imm), 2, "c.lw";
    CSwsp { rs2: IntReg, imm: i32 } => rv32ic_swsp(rs2 as i32, imm), 2, "c.swsp";
    CSw { rs1: IntReg, rs2: IntReg, imm: i32 } => rv32ic_sw(rs1 as i32, rs2 as i32, imm), 2, "c.sw";
    // rv64c
    CAddw { rd: IntReg, rs2: IntReg } => rv64ic_addw(rd as i32, rs2 as i32), 2, "c.addw";
    CSubw { rd: IntReg, rs2: IntReg } => rv64ic_subw(rd as i32, rs2 as i32), 2, "c.subw";
    CAddiw { rd: IntReg, imm: i32 } => rv64ic_addiw(rd as i32, imm), 2, "c.addiw";
    CLdsp { rd: IntReg, imm: i32 } => rv64ic_ldsp(rd as i32, imm), 2, "c.ldsp";
    CLd { rd: IntReg, rs1: IntReg, imm: i32 } => rv64ic_ld(rd as i32, rs1 as i32, imm), 2, "c.ld";
    CSdsp { rs2: IntReg, imm: i32 } => rv64ic_sdsp(rs2 as i32, imm), 2, "c.sdsp";
    CSd { rs1: IntReg, rs2: IntReg, imm: i32 } => rv64ic_sd(rs1 as i32, rs2 as i32, imm), 2, "c.sd";
    // zicsr
    Csrrw { rd: IntReg, rs1: IntReg, csr: i32 } => zicsr_csrrw(rd as i32, rs1 as i32, csr), 4, "csrrw", Csr;
    Csrrs { rd: IntReg, rs1: IntReg, csr: i32 } => zicsr_csrrs(rd as i32, rs1 as i32, csr), 4, "csrrs", Csr;
    Csrrc { rd: IntReg, rs1: IntReg, csr: i32 } => zicsr_csrrc(rd as i32, rs1 as i32, csr), 4, "csrrc", Csr;
    Csrrwi { rd: IntReg, uimm: i32, csr: i32 } => zicsr_csrrwi(rd as i32, uimm, csr), 4, "csrrwi", Csr;
    Csrrsi { rd: IntReg, uimm: i32, csr: i32 } => zicsr_csrrsi(rd as i32, uimm, csr), 4, "csrrsi", Csr;
    Csrrci { rd: IntReg, uimm: i32, csr: i32 } => zicsr_csrrci(rd as i32, uimm, csr), 4, "csrrci", Csr;
    // zifencei
    FenceI { imm: i32 } => zifencei_fencei(imm), 4, "fence.i", Fence;
    // rvprivileged
    Sret => rvprivileged_sret(), 4, "sret";
    Mret => rvprivileged_mret(), 4, "mret";
    Wfi => rvprivileged_wfi(), 4, "wfi";
    SfenceVma { rs1: IntReg, rs2: IntReg } => rvprivileged_sfence_vma(rs1 as i32, rs2 as i32), 4, "sfence.vma";
    SinvalVma { rs1: IntReg, rs2: IntReg } => rvprivileged_sinval_vma(rs1 as i32, rs2 as i32), 4, "sinval.vma";
    SfenceWInval => rvprivileged_sfence_w_inval(), 4, "sfence.w.inval";
    SfenceInvalIr => rvprivileged_sfence_inval_ir(), 4, "sfence.inval.ir";
}

impl Instruction {
    pub fn append_bytes(&self, bytes: &mut Vec<u8>) {
        self.encode().append_bytes(bytes);
    }

    /// Static dependency metadata when the existing RVWMO table has an entry.
    /// This is neither a dynamic dependency graph nor a memory-model checker.
    pub fn syntactic_dependency(&self) -> Option<&'static rvwmo::SyntacticDependency> {
        self.mnemonic().and_then(rvwmo::syntactic_dependency)
    }
}

impl From<EncodedInstruction> for Instruction {
    fn from(encoded: EncodedInstruction) -> Self {
        match encoded {
            EncodedInstruction::Standard(word) => Self::Standard(word),
            EncodedInstruction::Compressed(halfword) => Self::Compressed(halfword),
        }
    }
}

// Emit explicit-width directives so assembly and ELF output use identical bits
// without assembler relaxation or implicit compression.
impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.encode().fmt(f)
    }
}
