//! RISC-V instruction and test program generation.
//!
//! Weighted instruction/action selection lowers to operand-bearing instructions;
//! see [`generator`] for generation policy and program budgets.

pub mod cli;
pub mod elf;
pub mod generator;
pub mod riscv;
pub mod runtime;

pub use generator::{Generator, GeneratorParams};
pub use riscv::{EncodedInstruction, Instruction, InstructionClass, InstructionKind};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
