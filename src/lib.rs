//! RISC-V random program generator.
//!
//! The `rvgen` binary is a thin command-line wrapper over this library. To
//! generate a program in-process, fill in [`options::CommonOpts`] (its
//! `Default` is the CLI's defaults), including the instruction weights, and
//! call [`orchestrator::generate`]:
//!
//! ```no_run
//! use rvgen::{options::CommonOpts, orchestrator::generate, riscv::{InstructionClass, Opcode}};
//!
//! let mut opts = CommonOpts::default();
//! opts.seed = Some(42);
//! opts.class_weights.push((InstructionClass::Branch, 4.0));
//! opts.opcode_weights.push((Opcode::Jalr, 0.0));
//! let elf: Vec<u8> = generate(&opts).unwrap();
//! ```
mod basicblock;
pub mod csrs;
mod elf;
mod entangle;
mod hart;
mod membase;
mod privilege;
pub mod memory;
pub mod options;
pub mod orchestrator;
pub mod riscv;
mod spike;
pub mod target;
mod utils;
pub mod weights;
