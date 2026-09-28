use clap::Args;
use crate::riscv::instruction::{Opcode, Extension};

#[derive(Args, Clone)]
pub struct CommonOpts {
    // Total number of instructions for each core
    #[arg(long, default_value = "1000")]
    pub num_instrs: usize,

    // Number of cores of the generated program
    #[arg(long, default_value = "1")]
    pub num_cores: usize,

    // ISA Extension
    #[arg(long, default_value = "i", value_delimiter = ',')]
    pub isa: Vec<Extension>,

    // Disable specific instructions
    #[arg(long, default_value = "wfi", value_delimiter = ',')]
    pub disabled_instrs: Vec<Opcode>,
}

#[derive(Args, Clone)]
pub struct OneOpts {
    #[command(flatten)]
    pub common: CommonOpts,
    // Output file path
    #[arg(long, short = 'o', default_value = "rvprog.elf")]
    pub output: String,
}

#[derive(Args, Clone)]
pub struct ManyOpts {
    #[command(flatten)]
    pub common: CommonOpts,
    // Output directory path
    #[arg(long, short = 'o', default_value = "rvprogs")]
    pub outdir: String,
    // Number of ELF files to generate
    #[arg(long, default_value = "100")]
    pub num_elfs: u32,
}
