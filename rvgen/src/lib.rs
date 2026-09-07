//! Native Rust port of the rvgen Python project.
//!
//! Instruction encoding and ELF construction are implemented. Random program
//! generation retains the original class-selection scaffold; see [`generator`].

pub mod cli;
pub mod generator;
pub mod instgen;
pub mod riscv;
pub mod runtime;
pub mod utils;

pub use generator::{Generator, GeneratorParams};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
