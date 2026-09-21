// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only
//! The instruction catalog is the single source for variants, encoding,
//! metadata, and random construction. No per-instruction structs or trait objects.
use super::{encoding::*, fields::*, operand::Operand};
use rand::{Rng, RngExt};
use std::{fmt, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EncodedInstruction {
    Standard(u32),
    Compressed(u16),
}
impl EncodedInstruction {
    pub const fn bits(self) -> u32 {
        match self {
            Self::Standard(bits) => bits,
            Self::Compressed(bits) => bits as u32,
        }
    }
    pub const fn byte_len(self) -> usize {
        match self {
            Self::Standard(_) => 4,
            Self::Compressed(_) => 2,
        }
    }
    pub fn append_bytes(self, dst: &mut Vec<u8>) {
        match self {
            Self::Standard(bits) => dst.extend_from_slice(&bits.to_le_bytes()),
            Self::Compressed(bits) => dst.extend_from_slice(&bits.to_le_bytes()),
        }
    }
    pub fn to_le_bytes(self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.byte_len());
        self.append_bytes(&mut bytes);
        bytes
    }
}
impl fmt::Display for EncodedInstruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            ".{}byte 0x{:0width$x}",
            self.byte_len(),
            self.bits(),
            width = self.byte_len() * 2
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InstructionClass {
    Other,
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
    pub fn opcodes(self) -> impl Iterator<Item = Opcode> {
        Opcode::ALL
            .iter()
            .copied()
            .filter(move |op| op.class() == self)
    }
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
}

/// Extension containing an instruction. This does not model extension
/// prerequisites, enabled extensions, privilege, or architectural machine state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Extension {
    I,
    M,
    A,
    F,
    D,
    C,
    Zicsr,
    Zifencei,
    Privileged,
    Svinval,
    Raw,
}

#[derive(Debug, Clone, Copy)]
enum XlenRequirement {
    Both,
    Rv32,
    Rv64,
}
impl XlenRequirement {
    const fn supports(self, xlen: Xlen) -> bool {
        matches!(
            (self, xlen),
            (Self::Both, _) | (Self::Rv32, Xlen::X32) | (Self::Rv64, Xlen::X64)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncodeError {
    pub opcode: Opcode,
    pub field: &'static str,
    pub value: i64,
    pub requirement: &'static str,
}
impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}={} requires {}",
            self.opcode, self.field, self.value, self.requirement
        )
    }
}
impl std::error::Error for EncodeError {}

macro_rules! instructions {
    ($( $name:ident $( { $( $field:ident: $ty:ty ),* } )? =>
        ($mnemonic:literal, $class:ident, $extension:ident, $xlen:ident, $width:literal, $encoding:expr); )*) => {
        /// An editable instruction value, suitable for `Vec<Instruction>` or
        /// `Vec<Box<Instruction>>`. Invalid opcode/operand-shape combinations
        /// cannot be constructed. Immediates are checked by `encode`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Instruction {
            Raw32 { bits: u32 },
            Raw16 { bits: u16 },
            $( $name $( { $( $field: $ty ),* } )?, )*
        }
        /// An operand-free instruction identity, useful when selecting opcodes.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Opcode { Raw32, Raw16, $( $name, )* }

        impl Instruction {
            pub const fn opcode(&self) -> Opcode {
                match self {
                    Self::Raw32 { .. } => Opcode::Raw32,
                    Self::Raw16 { .. } => Opcode::Raw16,
                    $( Self::$name $( { $( $field: _ ),* } )? => Opcode::$name, )*
                }
            }
            fn encode_inner(&self, _xlen: Option<Xlen>) -> Result<EncodedInstruction, EncodeError> {
                match *self {
                    Self::Raw32 { bits } => Ok(EncodedInstruction::Standard(bits)),
                    Self::Raw16 { bits } => Ok(EncodedInstruction::Compressed(bits)),
                    $( Self::$name $( { $( $field ),* } )? => {
                        $( $(
                            $field.validate(Opcode::$name, stringify!($field), _xlen)?;
                            let $field = $field.bits();
                        )* )?
                        let bits: u32 = $encoding;
                        Ok(if $width == 2 {
                            // A bug in a compressed packer must never silently truncate.
                            EncodedInstruction::Compressed(u16::try_from(bits).expect("compressed packer overflow"))
                        } else { EncodedInstruction::Standard(bits) })
                    }, )*
                }
            }
        }
        impl Opcode {
            #[cfg(test)]
            pub(super) fn test_instruction(self, values: &[i64]) -> Instruction {
                let mut _values = values.iter().copied();
                let instruction = match self {
                    $(Self::$name => Instruction::$name $( { $(
                        $field: <$ty as super::tests::TestOperand>::from_value(_values.next().expect(stringify!($field)))
                    ),* } )?,)*
                    Self::Raw16 | Self::Raw32 => panic!("raw encodings are tested separately"),
                };
                assert!(_values.next().is_none(), "extra test operands for {self}");
                instruction
            }
            /// All named instructions, excluding the raw encoding escape hatches.
            pub const ALL: &'static [Self] = &[$( Self::$name, )*];
            pub const fn mnemonic(self) -> &'static str {
                match self { Self::Raw32 => ".4byte", Self::Raw16 => ".2byte", $(Self::$name => $mnemonic,)* }
            }
            pub const fn class(self) -> InstructionClass {
                match self {
                    Self::Raw32 | Self::Raw16 => InstructionClass::Other,
                    $(Self::$name => InstructionClass::$class,)*
                }
            }
            pub const fn extension(self) -> Extension {
                match self {
                    Self::Raw32 | Self::Raw16 => Extension::Raw,
                    $(Self::$name => Extension::$extension,)*
                }
            }
            pub const fn byte_len(self) -> usize {
                match self { Self::Raw32 => 4, Self::Raw16 => 2, $(Self::$name => $width,)* }
            }
            pub const fn supports_xlen(self, xlen: Xlen) -> bool {
                match self {
                    Self::Raw32 | Self::Raw16 => true,
                    $(Self::$name => XlenRequirement::$xlen.supports(xlen),)*
                }
            }
            /// Construct encodable operands. This does not arrange memory,
            /// control flow, privilege, or CSR state; execution can still trap.
            pub fn random(self, _rng: &mut (impl Rng + ?Sized), xlen: Xlen) -> Result<Instruction, EncodeError> {
                self.check_xlen(xlen)?;
                Ok(match self {
                    Self::Raw32 => Instruction::Raw32 { bits: _rng.random() },
                    Self::Raw16 => Instruction::Raw16 { bits: _rng.random() },
                    $(Self::$name => Instruction::$name $( { $(
                        $field: <$ty as Operand>::sample(self, stringify!($field), xlen, _rng)
                    ),* } )?,)*
                })
            }
        }
    };
}

// Entries: variant, mnemonic, generation class, extension, XLEN, byte width,
// encoding. Constant opcode/funct bits come from the reference implementations.
// Compressed packers follow the ISA; see README.md for corrected reference bugs.
instructions! {
    Lui { rd: XReg, imm: i32 } => ("lui", Alu, I, Both, 4, u(0x00000037, rd, imm));
    Auipc { rd: XReg, imm: i32 } => ("auipc", Alu, I, Both, 4, u(0x00000017, rd, imm));
    Jal { rd: XReg, imm: i32 } => ("jal", Jal, I, Both, 4, j(0x0000006f, rd, imm));
    Jalr { rd: XReg, rs1: XReg, imm: i32 } => ("jalr", Jalr, I, Both, 4, i(0x00000067, rd, rs1, imm));
    Beq { rs1: XReg, rs2: XReg, imm: i32 } => ("beq", Branch, I, Both, 4, b(0x00000063, rs1, rs2, imm));
    Bne { rs1: XReg, rs2: XReg, imm: i32 } => ("bne", Branch, I, Both, 4, b(0x00001063, rs1, rs2, imm));
    Blt { rs1: XReg, rs2: XReg, imm: i32 } => ("blt", Branch, I, Both, 4, b(0x00004063, rs1, rs2, imm));
    Bge { rs1: XReg, rs2: XReg, imm: i32 } => ("bge", Branch, I, Both, 4, b(0x00005063, rs1, rs2, imm));
    Bltu { rs1: XReg, rs2: XReg, imm: i32 } => ("bltu", Branch, I, Both, 4, b(0x00006063, rs1, rs2, imm));
    Bgeu { rs1: XReg, rs2: XReg, imm: i32 } => ("bgeu", Branch, I, Both, 4, b(0x00007063, rs1, rs2, imm));
    Lb { rd: XReg, rs1: XReg, imm: i32 } => ("lb", Memory, I, Both, 4, i(0x00000003, rd, rs1, imm));
    Lh { rd: XReg, rs1: XReg, imm: i32 } => ("lh", Memory, I, Both, 4, i(0x00001003, rd, rs1, imm));
    Lw { rd: XReg, rs1: XReg, imm: i32 } => ("lw", Memory, I, Both, 4, i(0x00002003, rd, rs1, imm));
    Lbu { rd: XReg, rs1: XReg, imm: i32 } => ("lbu", Memory, I, Both, 4, i(0x00004003, rd, rs1, imm));
    Lhu { rd: XReg, rs1: XReg, imm: i32 } => ("lhu", Memory, I, Both, 4, i(0x00005003, rd, rs1, imm));
    Sb { rs1: XReg, rs2: XReg, imm: i32 } => ("sb", Memory, I, Both, 4, s(0x00000023, rs1, rs2, imm));
    Sh { rs1: XReg, rs2: XReg, imm: i32 } => ("sh", Memory, I, Both, 4, s(0x00001023, rs1, rs2, imm));
    Sw { rs1: XReg, rs2: XReg, imm: i32 } => ("sw", Memory, I, Both, 4, s(0x00002023, rs1, rs2, imm));
    Addi { rd: XReg, rs1: XReg, imm: i32 } => ("addi", Alu, I, Both, 4, i(0x00000013, rd, rs1, imm));
    Slti { rd: XReg, rs1: XReg, imm: i32 } => ("slti", Alu, I, Both, 4, i(0x00002013, rd, rs1, imm));
    Sltiu { rd: XReg, rs1: XReg, imm: i32 } => ("sltiu", Alu, I, Both, 4, i(0x00003013, rd, rs1, imm));
    Xori { rd: XReg, rs1: XReg, imm: i32 } => ("xori", Alu, I, Both, 4, i(0x00004013, rd, rs1, imm));
    Ori { rd: XReg, rs1: XReg, imm: i32 } => ("ori", Alu, I, Both, 4, i(0x00006013, rd, rs1, imm));
    Andi { rd: XReg, rs1: XReg, imm: i32 } => ("andi", Alu, I, Both, 4, i(0x00007013, rd, rs1, imm));
    Slli { rd: XReg, rs1: XReg, shamt: u8 } => ("slli", Alu, I, Both, 4, i(0x00001013, rd, rs1, shamt));
    Srli { rd: XReg, rs1: XReg, shamt: u8 } => ("srli", Alu, I, Both, 4, i(0x00005013, rd, rs1, shamt));
    Srai { rd: XReg, rs1: XReg, shamt: u8 } => ("srai", Alu, I, Both, 4, i(0x40005013, rd, rs1, shamt));
    Add { rd: XReg, rs1: XReg, rs2: XReg } => ("add", Alu, I, Both, 4, r(0x00000033, rd, rs1, rs2));
    Sub { rd: XReg, rs1: XReg, rs2: XReg } => ("sub", Alu, I, Both, 4, r(0x40000033, rd, rs1, rs2));
    Sll { rd: XReg, rs1: XReg, rs2: XReg } => ("sll", Alu, I, Both, 4, r(0x00001033, rd, rs1, rs2));
    Slt { rd: XReg, rs1: XReg, rs2: XReg } => ("slt", Alu, I, Both, 4, r(0x00002033, rd, rs1, rs2));
    Sltu { rd: XReg, rs1: XReg, rs2: XReg } => ("sltu", Alu, I, Both, 4, r(0x00003033, rd, rs1, rs2));
    Xor { rd: XReg, rs1: XReg, rs2: XReg } => ("xor", Alu, I, Both, 4, r(0x00004033, rd, rs1, rs2));
    Srl { rd: XReg, rs1: XReg, rs2: XReg } => ("srl", Alu, I, Both, 4, r(0x00005033, rd, rs1, rs2));
    Sra { rd: XReg, rs1: XReg, rs2: XReg } => ("sra", Alu, I, Both, 4, r(0x40005033, rd, rs1, rs2));
    Or { rd: XReg, rs1: XReg, rs2: XReg } => ("or", Alu, I, Both, 4, r(0x00006033, rd, rs1, rs2));
    And { rd: XReg, rs1: XReg, rs2: XReg } => ("and", Alu, I, Both, 4, r(0x00007033, rd, rs1, rs2));
    Fence { imm: u16 } => ("fence", Fence, I, Both, 4, i(0x0000000f, 0, 0, imm));
    Ecall => ("ecall", Other, I, Both, 4, i(0x00000073, 0, 0, 0));
    Ebreak => ("ebreak", Other, I, Both, 4, i(0x00100073, 0, 0, 0));
    Lwu { rd: XReg, rs1: XReg, imm: i32 } => ("lwu", Memory64, I, Rv64, 4, i(0x00006003, rd, rs1, imm));
    Ld { rd: XReg, rs1: XReg, imm: i32 } => ("ld", Memory64, I, Rv64, 4, i(0x00003003, rd, rs1, imm));
    Sd { rs1: XReg, rs2: XReg, imm: i32 } => ("sd", Memory64, I, Rv64, 4, s(0x00003023, rs1, rs2, imm));
    Addiw { rd: XReg, rs1: XReg, imm: i32 } => ("addiw", Alu64, I, Rv64, 4, i(0x0000001b, rd, rs1, imm));
    Slliw { rd: XReg, rs1: XReg, shamt: u8 } => ("slliw", Alu64, I, Rv64, 4, i(0x0000101b, rd, rs1, shamt));
    Srliw { rd: XReg, rs1: XReg, shamt: u8 } => ("srliw", Alu64, I, Rv64, 4, i(0x0000501b, rd, rs1, shamt));
    Sraiw { rd: XReg, rs1: XReg, shamt: u8 } => ("sraiw", Alu64, I, Rv64, 4, i(0x4000501b, rd, rs1, shamt));
    Addw { rd: XReg, rs1: XReg, rs2: XReg } => ("addw", Alu64, I, Rv64, 4, r(0x0000003b, rd, rs1, rs2));
    Subw { rd: XReg, rs1: XReg, rs2: XReg } => ("subw", Alu64, I, Rv64, 4, r(0x4000003b, rd, rs1, rs2));
    Sllw { rd: XReg, rs1: XReg, rs2: XReg } => ("sllw", Alu64, I, Rv64, 4, r(0x0000103b, rd, rs1, rs2));
    Srlw { rd: XReg, rs1: XReg, rs2: XReg } => ("srlw", Alu64, I, Rv64, 4, r(0x0000503b, rd, rs1, rs2));
    Sraw { rd: XReg, rs1: XReg, rs2: XReg } => ("sraw", Alu64, I, Rv64, 4, r(0x4000503b, rd, rs1, rs2));
    Mul { rd: XReg, rs1: XReg, rs2: XReg } => ("mul", MulDiv, M, Both, 4, r(0x02000033, rd, rs1, rs2));
    Mulh { rd: XReg, rs1: XReg, rs2: XReg } => ("mulh", MulDiv, M, Both, 4, r(0x02001033, rd, rs1, rs2));
    Mulhsu { rd: XReg, rs1: XReg, rs2: XReg } => ("mulhsu", MulDiv, M, Both, 4, r(0x02002033, rd, rs1, rs2));
    Mulhu { rd: XReg, rs1: XReg, rs2: XReg } => ("mulhu", MulDiv, M, Both, 4, r(0x02003033, rd, rs1, rs2));
    Div { rd: XReg, rs1: XReg, rs2: XReg } => ("div", MulDiv, M, Both, 4, r(0x02004033, rd, rs1, rs2));
    Divu { rd: XReg, rs1: XReg, rs2: XReg } => ("divu", MulDiv, M, Both, 4, r(0x02005033, rd, rs1, rs2));
    Rem { rd: XReg, rs1: XReg, rs2: XReg } => ("rem", MulDiv, M, Both, 4, r(0x02006033, rd, rs1, rs2));
    Remu { rd: XReg, rs1: XReg, rs2: XReg } => ("remu", MulDiv, M, Both, 4, r(0x02007033, rd, rs1, rs2));
    Mulw { rd: XReg, rs1: XReg, rs2: XReg } => ("mulw", MulDiv64, M, Rv64, 4, r(0x0200003b, rd, rs1, rs2));
    Divw { rd: XReg, rs1: XReg, rs2: XReg } => ("divw", MulDiv64, M, Rv64, 4, r(0x0200403b, rd, rs1, rs2));
    Divuw { rd: XReg, rs1: XReg, rs2: XReg } => ("divuw", MulDiv64, M, Rv64, 4, r(0x0200503b, rd, rs1, rs2));
    Remw { rd: XReg, rs1: XReg, rs2: XReg } => ("remw", MulDiv64, M, Rv64, 4, r(0x0200603b, rd, rs1, rs2));
    Remuw { rd: XReg, rs1: XReg, rs2: XReg } => ("remuw", MulDiv64, M, Rv64, 4, r(0x0200703b, rd, rs1, rs2));
    LrW { ordering: Ordering, rd: XReg, rs1: XReg } => ("lr.w", Amo, A, Both, 4, r(0x1000202f, rd, rs1, 0) | (ordering << 25));
    ScW { ordering: Ordering, rd: XReg, rs1: XReg, rs2: XReg } => ("sc.w", Amo, A, Both, 4, r(0x1800202f, rd, rs1, rs2) | (ordering << 25));
    AmoswapW { ordering: Ordering, rd: XReg, rs1: XReg, rs2: XReg } => ("amoswap.w", Amo, A, Both, 4, r(0x0800202f, rd, rs1, rs2) | (ordering << 25));
    AmoaddW { ordering: Ordering, rd: XReg, rs1: XReg, rs2: XReg } => ("amoadd.w", Amo, A, Both, 4, r(0x0000202f, rd, rs1, rs2) | (ordering << 25));
    AmoandW { ordering: Ordering, rd: XReg, rs1: XReg, rs2: XReg } => ("amoand.w", Amo, A, Both, 4, r(0x6000202f, rd, rs1, rs2) | (ordering << 25));
    AmoorW { ordering: Ordering, rd: XReg, rs1: XReg, rs2: XReg } => ("amoor.w", Amo, A, Both, 4, r(0x4000202f, rd, rs1, rs2) | (ordering << 25));
    AmoxorW { ordering: Ordering, rd: XReg, rs1: XReg, rs2: XReg } => ("amoxor.w", Amo, A, Both, 4, r(0x2000202f, rd, rs1, rs2) | (ordering << 25));
    AmomaxW { ordering: Ordering, rd: XReg, rs1: XReg, rs2: XReg } => ("amomax.w", Amo, A, Both, 4, r(0xa000202f, rd, rs1, rs2) | (ordering << 25));
    AmomaxuW { ordering: Ordering, rd: XReg, rs1: XReg, rs2: XReg } => ("amomaxu.w", Amo, A, Both, 4, r(0xe000202f, rd, rs1, rs2) | (ordering << 25));
    AmominW { ordering: Ordering, rd: XReg, rs1: XReg, rs2: XReg } => ("amomin.w", Amo, A, Both, 4, r(0x8000202f, rd, rs1, rs2) | (ordering << 25));
    AmominuW { ordering: Ordering, rd: XReg, rs1: XReg, rs2: XReg } => ("amominu.w", Amo, A, Both, 4, r(0xc000202f, rd, rs1, rs2) | (ordering << 25));
    LrD { ordering: Ordering, rd: XReg, rs1: XReg } => ("lr.d", Amo64, A, Rv64, 4, r(0x1000302f, rd, rs1, 0) | (ordering << 25));
    ScD { ordering: Ordering, rd: XReg, rs1: XReg, rs2: XReg } => ("sc.d", Amo64, A, Rv64, 4, r(0x1800302f, rd, rs1, rs2) | (ordering << 25));
    AmoswapD { ordering: Ordering, rd: XReg, rs1: XReg, rs2: XReg } => ("amoswap.d", Amo64, A, Rv64, 4, r(0x0800302f, rd, rs1, rs2) | (ordering << 25));
    AmoaddD { ordering: Ordering, rd: XReg, rs1: XReg, rs2: XReg } => ("amoadd.d", Amo64, A, Rv64, 4, r(0x0000302f, rd, rs1, rs2) | (ordering << 25));
    AmoandD { ordering: Ordering, rd: XReg, rs1: XReg, rs2: XReg } => ("amoand.d", Amo64, A, Rv64, 4, r(0x6000302f, rd, rs1, rs2) | (ordering << 25));
    AmoorD { ordering: Ordering, rd: XReg, rs1: XReg, rs2: XReg } => ("amoor.d", Amo64, A, Rv64, 4, r(0x4000302f, rd, rs1, rs2) | (ordering << 25));
    AmoxorD { ordering: Ordering, rd: XReg, rs1: XReg, rs2: XReg } => ("amoxor.d", Amo64, A, Rv64, 4, r(0x2000302f, rd, rs1, rs2) | (ordering << 25));
    AmomaxD { ordering: Ordering, rd: XReg, rs1: XReg, rs2: XReg } => ("amomax.d", Amo64, A, Rv64, 4, r(0xa000302f, rd, rs1, rs2) | (ordering << 25));
    AmomaxuD { ordering: Ordering, rd: XReg, rs1: XReg, rs2: XReg } => ("amomaxu.d", Amo64, A, Rv64, 4, r(0xe000302f, rd, rs1, rs2) | (ordering << 25));
    AmominD { ordering: Ordering, rd: XReg, rs1: XReg, rs2: XReg } => ("amomin.d", Amo64, A, Rv64, 4, r(0x8000302f, rd, rs1, rs2) | (ordering << 25));
    AmominuD { ordering: Ordering, rd: XReg, rs1: XReg, rs2: XReg } => ("amominu.d", Amo64, A, Rv64, 4, r(0xc000302f, rd, rs1, rs2) | (ordering << 25));
    Flw { rd: FReg, rs1: XReg, imm: i32 } => ("flw", FloatMemory, F, Both, 4, i(0x00002007, rd, rs1, imm));
    Fsw { rs1: XReg, rs2: FReg, imm: i32 } => ("fsw", FloatMemory, F, Both, 4, s(0x00002027, rs1, rs2, imm));
    FmaddS { rd: FReg, rs1: FReg, rs2: FReg, rs3: FReg, rm: RoundingMode } => ("fmadd.s", Float, F, Both, 4, r4(0x00000043, rd, rs1, rs2, rs3) | (rm << 12));
    FmsubS { rd: FReg, rs1: FReg, rs2: FReg, rs3: FReg, rm: RoundingMode } => ("fmsub.s", Float, F, Both, 4, r4(0x00000047, rd, rs1, rs2, rs3) | (rm << 12));
    FnmsubS { rd: FReg, rs1: FReg, rs2: FReg, rs3: FReg, rm: RoundingMode } => ("fnmsub.s", Float, F, Both, 4, r4(0x0000004b, rd, rs1, rs2, rs3) | (rm << 12));
    FnmaddS { rd: FReg, rs1: FReg, rs2: FReg, rs3: FReg, rm: RoundingMode } => ("fnmadd.s", Float, F, Both, 4, r4(0x0000004f, rd, rs1, rs2, rs3) | (rm << 12));
    FaddS { rd: FReg, rs1: FReg, rs2: FReg, rm: RoundingMode } => ("fadd.s", Float, F, Both, 4, r(0x00000053, rd, rs1, rs2) | (rm << 12));
    FsubS { rd: FReg, rs1: FReg, rs2: FReg, rm: RoundingMode } => ("fsub.s", Float, F, Both, 4, r(0x08000053, rd, rs1, rs2) | (rm << 12));
    FmulS { rd: FReg, rs1: FReg, rs2: FReg, rm: RoundingMode } => ("fmul.s", Float, F, Both, 4, r(0x10000053, rd, rs1, rs2) | (rm << 12));
    FdivS { rd: FReg, rs1: FReg, rs2: FReg, rm: RoundingMode } => ("fdiv.s", Float, F, Both, 4, r(0x18000053, rd, rs1, rs2) | (rm << 12));
    FsqrtS { rd: FReg, rs1: FReg, rm: RoundingMode } => ("fsqrt.s", Float, F, Both, 4, r(0x58000053, rd, rs1, 0) | (rm << 12));
    FsgnjS { rd: FReg, rs1: FReg, rs2: FReg } => ("fsgnj.s", Float, F, Both, 4, r(0x20000053, rd, rs1, rs2));
    FsgnjnS { rd: FReg, rs1: FReg, rs2: FReg } => ("fsgnjn.s", Float, F, Both, 4, r(0x20001053, rd, rs1, rs2));
    FsgnjxS { rd: FReg, rs1: FReg, rs2: FReg } => ("fsgnjx.s", Float, F, Both, 4, r(0x20002053, rd, rs1, rs2));
    FminS { rd: FReg, rs1: FReg, rs2: FReg } => ("fmin.s", Float, F, Both, 4, r(0x28000053, rd, rs1, rs2));
    FmaxS { rd: FReg, rs1: FReg, rs2: FReg } => ("fmax.s", Float, F, Both, 4, r(0x28001053, rd, rs1, rs2));
    FcvtWS { rd: XReg, rs1: FReg, rm: RoundingMode } => ("fcvt.w.s", Float, F, Both, 4, r(0xc0000053, rd, rs1, 0) | (rm << 12));
    FcvtWuS { rd: XReg, rs1: FReg, rm: RoundingMode } => ("fcvt.wu.s", Float, F, Both, 4, r(0xc0100053, rd, rs1, 0) | (rm << 12));
    FmvXW { rd: XReg, rs1: FReg } => ("fmv.x.w", Float, F, Both, 4, r(0xe0000053, rd, rs1, 0));
    FeqS { rd: XReg, rs1: FReg, rs2: FReg } => ("feq.s", Float, F, Both, 4, r(0xa0002053, rd, rs1, rs2));
    FltS { rd: XReg, rs1: FReg, rs2: FReg } => ("flt.s", Float, F, Both, 4, r(0xa0001053, rd, rs1, rs2));
    FleS { rd: XReg, rs1: FReg, rs2: FReg } => ("fle.s", Float, F, Both, 4, r(0xa0000053, rd, rs1, rs2));
    FclassS { rd: XReg, rs1: FReg } => ("fclass.s", Float, F, Both, 4, r(0xe0001053, rd, rs1, 0));
    FcvtSW { rd: FReg, rs1: XReg, rm: RoundingMode } => ("fcvt.s.w", Float, F, Both, 4, r(0xd0000053, rd, rs1, 0) | (rm << 12));
    FcvtSWu { rd: FReg, rs1: XReg, rm: RoundingMode } => ("fcvt.s.wu", Float, F, Both, 4, r(0xd0100053, rd, rs1, 0) | (rm << 12));
    FmvWX { rd: FReg, rs1: XReg } => ("fmv.w.x", Float, F, Both, 4, r(0xf0000053, rd, rs1, 0));
    FcvtLS { rd: XReg, rs1: FReg, rm: RoundingMode } => ("fcvt.l.s", Float64, F, Rv64, 4, r(0xc0200053, rd, rs1, 0) | (rm << 12));
    FcvtLuS { rd: XReg, rs1: FReg, rm: RoundingMode } => ("fcvt.lu.s", Float64, F, Rv64, 4, r(0xc0300053, rd, rs1, 0) | (rm << 12));
    FcvtSL { rd: FReg, rs1: XReg, rm: RoundingMode } => ("fcvt.s.l", Float64, F, Rv64, 4, r(0xd0200053, rd, rs1, 0) | (rm << 12));
    FcvtSLu { rd: FReg, rs1: XReg, rm: RoundingMode } => ("fcvt.s.lu", Float64, F, Rv64, 4, r(0xd0300053, rd, rs1, 0) | (rm << 12));
    Fld { rd: FReg, rs1: XReg, imm: i32 } => ("fld", DoubleMemory, D, Both, 4, i(0x00003007, rd, rs1, imm));
    Fsd { rs1: XReg, rs2: FReg, imm: i32 } => ("fsd", DoubleMemory, D, Both, 4, s(0x00003027, rs1, rs2, imm));
    FmaddD { rd: FReg, rs1: FReg, rs2: FReg, rs3: FReg, rm: RoundingMode } => ("fmadd.d", Double, D, Both, 4, r4(0x02000043, rd, rs1, rs2, rs3) | (rm << 12));
    FmsubD { rd: FReg, rs1: FReg, rs2: FReg, rs3: FReg, rm: RoundingMode } => ("fmsub.d", Double, D, Both, 4, r4(0x02000047, rd, rs1, rs2, rs3) | (rm << 12));
    FnmsubD { rd: FReg, rs1: FReg, rs2: FReg, rs3: FReg, rm: RoundingMode } => ("fnmsub.d", Double, D, Both, 4, r4(0x0200004b, rd, rs1, rs2, rs3) | (rm << 12));
    FnmaddD { rd: FReg, rs1: FReg, rs2: FReg, rs3: FReg, rm: RoundingMode } => ("fnmadd.d", Double, D, Both, 4, r4(0x0200004f, rd, rs1, rs2, rs3) | (rm << 12));
    FaddD { rd: FReg, rs1: FReg, rs2: FReg, rm: RoundingMode } => ("fadd.d", Double, D, Both, 4, r(0x02000053, rd, rs1, rs2) | (rm << 12));
    FsubD { rd: FReg, rs1: FReg, rs2: FReg, rm: RoundingMode } => ("fsub.d", Double, D, Both, 4, r(0x0a000053, rd, rs1, rs2) | (rm << 12));
    FmulD { rd: FReg, rs1: FReg, rs2: FReg, rm: RoundingMode } => ("fmul.d", Double, D, Both, 4, r(0x12000053, rd, rs1, rs2) | (rm << 12));
    FdivD { rd: FReg, rs1: FReg, rs2: FReg, rm: RoundingMode } => ("fdiv.d", Double, D, Both, 4, r(0x1a000053, rd, rs1, rs2) | (rm << 12));
    FsqrtD { rd: FReg, rs1: FReg, rm: RoundingMode } => ("fsqrt.d", Double, D, Both, 4, r(0x5a000053, rd, rs1, 0) | (rm << 12));
    FsgnjD { rd: FReg, rs1: FReg, rs2: FReg } => ("fsgnj.d", Double, D, Both, 4, r(0x22000053, rd, rs1, rs2));
    FsgnjnD { rd: FReg, rs1: FReg, rs2: FReg } => ("fsgnjn.d", Double, D, Both, 4, r(0x22001053, rd, rs1, rs2));
    FsgnjxD { rd: FReg, rs1: FReg, rs2: FReg } => ("fsgnjx.d", Double, D, Both, 4, r(0x22002053, rd, rs1, rs2));
    FminD { rd: FReg, rs1: FReg, rs2: FReg } => ("fmin.d", Double, D, Both, 4, r(0x2a000053, rd, rs1, rs2));
    FmaxD { rd: FReg, rs1: FReg, rs2: FReg } => ("fmax.d", Double, D, Both, 4, r(0x2a001053, rd, rs1, rs2));
    FcvtSD { rd: FReg, rs1: FReg, rm: RoundingMode } => ("fcvt.s.d", Double, D, Both, 4, r(0x40100053, rd, rs1, 0) | (rm << 12));
    FcvtDS { rd: FReg, rs1: FReg, rm: RoundingMode } => ("fcvt.d.s", Double, D, Both, 4, r(0x42000053, rd, rs1, 0) | (rm << 12));
    FeqD { rd: XReg, rs1: FReg, rs2: FReg } => ("feq.d", Double, D, Both, 4, r(0xa2002053, rd, rs1, rs2));
    FltD { rd: XReg, rs1: FReg, rs2: FReg } => ("flt.d", Double, D, Both, 4, r(0xa2001053, rd, rs1, rs2));
    FleD { rd: XReg, rs1: FReg, rs2: FReg } => ("fle.d", Double, D, Both, 4, r(0xa2000053, rd, rs1, rs2));
    FclassD { rd: XReg, rs1: FReg } => ("fclass.d", Double, D, Both, 4, r(0xe2001053, rd, rs1, 0));
    FcvtWD { rd: XReg, rs1: FReg, rm: RoundingMode } => ("fcvt.w.d", Double, D, Both, 4, r(0xc2000053, rd, rs1, 0) | (rm << 12));
    FcvtWuD { rd: XReg, rs1: FReg, rm: RoundingMode } => ("fcvt.wu.d", Double, D, Both, 4, r(0xc2100053, rd, rs1, 0) | (rm << 12));
    FcvtDW { rd: FReg, rs1: XReg, rm: RoundingMode } => ("fcvt.d.w", Double, D, Both, 4, r(0xd2000053, rd, rs1, 0) | (rm << 12));
    FcvtDWu { rd: FReg, rs1: XReg, rm: RoundingMode } => ("fcvt.d.wu", Double, D, Both, 4, r(0xd2100053, rd, rs1, 0) | (rm << 12));
    FcvtLD { rd: XReg, rs1: FReg, rm: RoundingMode } => ("fcvt.l.d", Double64, D, Rv64, 4, r(0xc2200053, rd, rs1, 0) | (rm << 12));
    FcvtLuD { rd: XReg, rs1: FReg, rm: RoundingMode } => ("fcvt.lu.d", Double64, D, Rv64, 4, r(0xc2300053, rd, rs1, 0) | (rm << 12));
    FmvXD { rd: XReg, rs1: FReg } => ("fmv.x.d", Double64, D, Rv64, 4, r(0xe2000053, rd, rs1, 0));
    FcvtDL { rd: FReg, rs1: XReg, rm: RoundingMode } => ("fcvt.d.l", Double64, D, Rv64, 4, r(0xd2200053, rd, rs1, 0) | (rm << 12));
    FcvtDLu { rd: FReg, rs1: XReg, rm: RoundingMode } => ("fcvt.d.lu", Double64, D, Rv64, 4, r(0xd2300053, rd, rs1, 0) | (rm << 12));
    FmvDX { rd: FReg, rs1: XReg } => ("fmv.d.x", Double64, D, Rv64, 4, r(0xf2000053, rd, rs1, 0));
    CMv { rd: XReg, rs2: XReg } => ("c.mv", Alu, C, Both, 2, cr(0x8002, rd, rs2));
    CAdd { rd: XReg, rs2: XReg } => ("c.add", Alu, C, Both, 2, cr(0x9002, rd, rs2));
    CAnd { rd: XReg, rs2: XReg } => ("c.and", Alu, C, Both, 2, ca(0x8c61, rd, rs2));
    COr { rd: XReg, rs2: XReg } => ("c.or", Alu, C, Both, 2, ca(0x8c41, rd, rs2));
    CXor { rd: XReg, rs2: XReg } => ("c.xor", Alu, C, Both, 2, ca(0x8c21, rd, rs2));
    CSub { rd: XReg, rs2: XReg } => ("c.sub", Alu, C, Both, 2, ca(0x8c01, rd, rs2));
    CLui { rd: XReg, imm: i32 } => ("c.lui", Alu, C, Both, 2, ci(0x6001, rd, imm));
    CAddi16sp { imm: i32 } => ("c.addi16sp", Alu, C, Both, 2, ci(0x6001, 2, addi16sp_imm(imm)));
    CAddi4spn { rd: XReg, imm: i32 } => ("c.addi4spn", Alu, C, Both, 2, ciw(rd, imm));
    CAddi { rd: XReg, imm: i32 } => ("c.addi", Alu, C, Both, 2, ci(0x0001, rd, imm));
    CLi { rd: XReg, imm: i32 } => ("c.li", Alu, C, Both, 2, ci(0x4001, rd, imm));
    CSlli { rd: XReg, shamt: u8 } => ("c.slli", Alu, C, Both, 2, ci(0x0002, rd, shamt));
    CAndi { rs1: XReg, imm: i32 } => ("c.andi", Alu, C, Both, 2, cb_alu(0x8801, rs1, imm));
    CSrli { rs1: XReg, shamt: u8 } => ("c.srli", Alu, C, Both, 2, cb_alu(0x8001, rs1, shamt));
    CSrai { rs1: XReg, shamt: u8 } => ("c.srai", Alu, C, Both, 2, cb_alu(0x8401, rs1, shamt));
    CBeqz { rs1: XReg, imm: i32 } => ("c.beqz", Branch, C, Both, 2, cb(0xc001, rs1, imm));
    CBnez { rs1: XReg, imm: i32 } => ("c.bnez", Branch, C, Both, 2, cb(0xe001, rs1, imm));
    CJal { imm: i32 } => ("c.jal", Jal, C, Rv32, 2, cj(0x2001, imm));
    CJ { imm: i32 } => ("c.j", Jal, C, Both, 2, cj(0xa001, imm));
    CJr { rs1: XReg } => ("c.jr", Jalr, C, Both, 2, cr(0x8002, rs1, 0));
    CJalr { rs1: XReg } => ("c.jalr", Jalr, C, Both, 2, cr(0x9002, rs1, 0));
    CEbreak => ("c.ebreak", Other, C, Both, 2, 0x9002);
    CLwsp { rd: XReg, imm: i32 } => ("c.lwsp", Memory, C, Both, 2, ci(0x4002, rd, lwsp_imm(imm)));
    CLw { rd: XReg, rs1: XReg, imm: i32 } => ("c.lw", Memory, C, Both, 2, cl(0x4000, rd, rs1, word_imm(imm)));
    CSwsp { rs2: XReg, imm: i32 } => ("c.swsp", Memory, C, Both, 2, css(0xc002, rs2, swsp_imm(imm)));
    CSw { rs1: XReg, rs2: XReg, imm: i32 } => ("c.sw", Memory, C, Both, 2, cl(0xc000, rs2, rs1, word_imm(imm)));
    CAddw { rd: XReg, rs2: XReg } => ("c.addw", Alu64, C, Rv64, 2, ca(0x9c21, rd, rs2));
    CSubw { rd: XReg, rs2: XReg } => ("c.subw", Alu64, C, Rv64, 2, ca(0x9c01, rd, rs2));
    CAddiw { rd: XReg, imm: i32 } => ("c.addiw", Alu64, C, Rv64, 2, ci(0x2001, rd, imm));
    CLdsp { rd: XReg, imm: i32 } => ("c.ldsp", Memory64, C, Rv64, 2, ci(0x6002, rd, ldsp_imm(imm)));
    CLd { rd: XReg, rs1: XReg, imm: i32 } => ("c.ld", Memory64, C, Rv64, 2, cl(0x6000, rd, rs1, double_imm(imm)));
    CSdsp { rs2: XReg, imm: i32 } => ("c.sdsp", Memory64, C, Rv64, 2, css(0xe002, rs2, sdsp_imm(imm)));
    CSd { rs1: XReg, rs2: XReg, imm: i32 } => ("c.sd", Memory64, C, Rv64, 2, cl(0xe000, rs2, rs1, double_imm(imm)));
    Csrrw { rd: XReg, rs1: XReg, csr: Csr } => ("csrrw", Csr, Zicsr, Both, 4, i(0x00001073, rd, rs1, csr));
    Csrrs { rd: XReg, rs1: XReg, csr: Csr } => ("csrrs", Csr, Zicsr, Both, 4, i(0x00002073, rd, rs1, csr));
    Csrrc { rd: XReg, rs1: XReg, csr: Csr } => ("csrrc", Csr, Zicsr, Both, 4, i(0x00003073, rd, rs1, csr));
    Csrrwi { rd: XReg, uimm: u8, csr: Csr } => ("csrrwi", Csr, Zicsr, Both, 4, i(0x00005073, rd, uimm, csr));
    Csrrsi { rd: XReg, uimm: u8, csr: Csr } => ("csrrsi", Csr, Zicsr, Both, 4, i(0x00006073, rd, uimm, csr));
    Csrrci { rd: XReg, uimm: u8, csr: Csr } => ("csrrci", Csr, Zicsr, Both, 4, i(0x00007073, rd, uimm, csr));
    FenceI => ("fence.i", Fence, Zifencei, Both, 4, 0x0000100f);
    Sret => ("sret", Other, Privileged, Both, 4, r(0x10200073, 0, 0, 0));
    Mret => ("mret", Other, Privileged, Both, 4, r(0x30200073, 0, 0, 0));
    Wfi => ("wfi", Other, Privileged, Both, 4, r(0x10500073, 0, 0, 0));
    SfenceVma { rs1: XReg, rs2: XReg } => ("sfence.vma", Other, Privileged, Both, 4, r(0x12000073, 0, rs1, rs2));
    SinvalVma { rs1: XReg, rs2: XReg } => ("sinval.vma", Other, Svinval, Both, 4, r(0x16000073, 0, rs1, rs2));
    SfenceWInval => ("sfence.w.inval", Other, Svinval, Both, 4, r(0x18000073, 0, 0, 0));
    SfenceInvalIr => ("sfence.inval.ir", Other, Svinval, Both, 4, r(0x18100073, 0, 0, 0));
}

impl Instruction {
    pub const fn standard(bits: u32) -> Self {
        Self::Raw32 { bits }
    }
    pub const fn compressed(bits: u16) -> Self {
        Self::Raw16 { bits }
    }
    pub const fn nop() -> Self {
        Self::Addi {
            rd: XReg::ZERO,
            rs1: XReg::ZERO,
            imm: 0,
        }
    }
    pub const fn cnop() -> Self {
        Self::CAddi {
            rd: XReg::ZERO,
            imm: 0,
        }
    }
    pub const fn mnemonic(&self) -> &'static str {
        self.opcode().mnemonic()
    }
    pub const fn class(&self) -> InstructionClass {
        self.opcode().class()
    }
    pub const fn byte_len(&self) -> usize {
        self.opcode().byte_len()
    }

    /// Encode with operand checks but no target XLEN restriction. For example,
    /// SLLI with a shift of 63 is allowed here; use `encode_for` for RV32 checks.
    pub fn encode(&self) -> Result<EncodedInstruction, EncodeError> {
        self.encode_inner(None)
    }
    /// Check target XLEN and operands. Enabled extensions and privilege are the
    /// caller's responsibility. Raw encodings bypass operand and XLEN checks.
    pub fn encode_for(&self, xlen: Xlen) -> Result<EncodedInstruction, EncodeError> {
        self.opcode().check_xlen(xlen)?;
        self.encode_inner(Some(xlen))
    }
    /// Append a little-endian instruction. On error `dst` is unchanged.
    pub fn append_bytes(&self, dst: &mut Vec<u8>) -> Result<(), EncodeError> {
        self.encode()?.append_bytes(dst);
        Ok(())
    }
    pub fn syntactic_dependency(&self) -> Option<super::rvwmo::SyntacticDependency> {
        super::rvwmo::syntactic_dependency_for(self.mnemonic())
    }
}
impl Opcode {
    fn check_xlen(self, xlen: Xlen) -> Result<(), EncodeError> {
        if self.supports_xlen(xlen) {
            Ok(())
        } else {
            Err(EncodeError {
                opcode: self,
                field: "XLEN",
                value: xlen.bits() as i64,
                requirement: if self == Self::CJal { "RV32" } else { "RV64" },
            })
        }
    }
}
impl fmt::Display for Opcode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.mnemonic())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownOpcode;
impl fmt::Display for UnknownOpcode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unknown RISC-V mnemonic")
    }
}
impl std::error::Error for UnknownOpcode {}
impl FromStr for Opcode {
    type Err = UnknownOpcode;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .copied()
            .chain([Self::Raw32, Self::Raw16])
            .find(|op| op.mnemonic() == value)
            .ok_or(UnknownOpcode)
    }
}
