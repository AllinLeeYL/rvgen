//! RISC-V instructions for RV32/RV64 I, M, A, F, D, C, Zicsr, Zifencei and
//! the privileged operations present in the Python/Go references.
//!
//! `Instruction` is a small, editable value: store `Vec<Instruction>` in a
//! basic block, or retain `Vec<Box<Instruction>>` if owning pointers are useful.
//! Each variant carries exactly its operands, with distinct integer/FP registers.
//! See `README.md` in this directory for construction and generation examples.

// The binary has not connected instruction generation yet. Keep the public
// module usable without a warning for every not-yet-used opcode.
#![allow(dead_code, unused_imports)]

pub mod asmutil;
mod encoding;
pub mod fields;
pub mod instruction;
mod operand;
pub mod registers;
pub mod rvwmo;

pub use fields::{Csr, FReg, FenceOrdering, OperandError, Ordering, RoundingMode, XReg, Xlen};
pub use instruction::{
    EncodeError, EncodedInstruction, Extension, Instruction, InstructionClass, Opcode,
};

#[cfg(test)]
mod tests;
