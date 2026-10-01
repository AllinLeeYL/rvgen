use crate::riscv::{Extension, Opcode, Xlen};
use clap::Args;

#[derive(Args, Clone)]
pub struct CommonOpts {
    // Total number of instructions for each core
    #[arg(long, default_value = "1000")]
    pub num_instrs: usize,

    // Number of cores of the generated program
    #[arg(long, default_value = "1")]
    pub num_cores: usize,

    // ISA Extension
    #[arg(long, default_value = "i,zicsr", value_delimiter = ',')]
    pub isa: Vec<Extension>,

    /// Target register width (32 or 64).
    #[arg(long, default_value = "64", value_parser = parse_xlen)]
    pub xlen: Xlen,

    /// Physical RAM base; decimal or hexadecimal. Also the ELF entry address.
    #[arg(long, default_value = "0x80000000", value_parser = parse_address)]
    pub ram_base: u64,

    /// Physical RAM size in bytes; must match the simulator/board configuration.
    #[arg(long, default_value = "0x08000000", value_parser = parse_address)]
    pub ram_size: u64,

    /// Writable memory reserved for generated loads, stores, and atomics.
    #[arg(long, default_value = "4096")]
    pub scratch_size: usize,

    /// Writable/executable SMC section size in bytes; zero omits the section.
    #[arg(long, default_value = "4096")]
    pub smc_size: usize,

    // Disable specific instructions
    #[arg(long, default_value = "wfi,lr.w,lr.d", value_delimiter = ',')]
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
    #[arg(long, short = 'o', default_value = "elfs")]
    pub outdir: String,
    // Number of ELF files to generate
    #[arg(long, default_value = "100")]
    pub num_elfs: u32,
}

fn parse_address(value: &str) -> Result<u64, String> {
    let parsed = if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16)
    } else {
        value.parse()
    };
    parsed.map_err(|error| error.to_string())
}

fn parse_xlen(value: &str) -> Result<Xlen, String> {
    match value {
        "32" => Ok(Xlen::X32),
        "64" => Ok(Xlen::X64),
        _ => Err("XLEN must be 32 or 64".into()),
    }
}
