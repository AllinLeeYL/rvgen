//! RISC-V instructions, registers, architectural metadata, and bit encoders.
//!
//! [`Instruction`] provides typed operands. [`encoding`] contains the low-level
//! helpers, also re-exported here for direct calls such as [`rv32i_add`].

pub mod asmutil;
pub mod csrids;
pub mod encoding;
pub mod instruction;
pub mod mappedregs;
pub mod registers;
pub mod rvwmo;
pub mod util;

pub use csrids::*;
pub use encoding::*;
pub use instruction::{EncodedInstruction, Instruction, InstructionClass, InstructionKind};
pub use mappedregs::*;
pub use registers::*;
pub use util::*;
