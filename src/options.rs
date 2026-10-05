use crate::riscv::{Extension, InstructionClass, Opcode, PrivilegeLevel, Xlen};
use crate::weights::InstrWeights;
use clap::{Args, Parser};
use std::fmt::Display;
use std::str::FromStr;

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

    // Target register width (32 or 64).
    #[arg(long, default_value = "64", value_parser = parse_xlen)]
    pub xlen: Xlen,

    // Target privilege modes (m, s, u); M is required and S requires U.
    #[arg(long = "priv", default_value = "m,s,u", value_delimiter = ',')]
    pub privileges: Vec<PrivilegeLevel>,

    // Physical RAM base; decimal or hexadecimal. Also the ELF entry address.
    #[arg(long, default_value = "0x80000000", value_parser = parse_address)]
    pub ram_base: u64,

    // Physical RAM size in bytes; must match the simulator/board configuration.
    #[arg(long, default_value = "0x08000000", value_parser = parse_address)]
    pub ram_size: u64,

    // Writable memory reserved for generated loads, stores, and atomics, in
    // bytes; drawn per program between 4 KiB and 1 MiB when omitted.
    #[arg(long, value_parser = parse_address)]
    pub scratch_size: Option<u64>,

    // Writable/executable SMC section size in bytes; zero omits the section.
    #[arg(long, default_value = "8192")]
    pub smc_size: u64,

    // PMP entries the target implements (at most 16); `_init` configures
    // them, never locked. 0 leaves PMP untouched.
    #[arg(long, default_value = "8")]
    pub pmp_regions: usize,

    // Disable specific instructions
    #[arg(long, default_value = "wfi,lr.w,lr.d", value_delimiter = ',')]
    pub disabled_instrs: Vec<Opcode>,

    /// Skip the self-check: the program then reports success whenever it
    /// reaches the end, and Spike is not needed.
    #[arg(long)]
    pub no_self_check: bool,

    /// Do not entangle memory addresses and control flow with the workload's
    /// data. Entanglement needs Spike, so `--no-self-check` implies this.
    #[arg(long)]
    pub no_entangle: bool,

    /// Also guard mid-block once this many workload-written registers are
    /// unchecked; 0 guards at block ends only. Lower catches more faults
    /// before they are overwritten, at the cost of longer programs.
    #[arg(long, default_value = "8")]
    pub guard_threshold: usize,

    /// Materialize the code's constants inline instead of loading them from
    /// the `.golden` data section: each entanglement site's golden value (its
    /// Spike-decided constant) and `_check`'s expected values with an
    /// 8-instruction sequence, `_init`'s values with up to 8. RV32 always does
    /// this.
    #[arg(long)]
    pub inline_golden: bool,

    /// Spike executable that computes the self-check's expected values.
    #[arg(long, default_value = "spike")]
    pub spike: String,

    /// Relative weight of an opcode in the workload, as `MNEMONIC=WEIGHT`.
    /// Repeat the flag or comma-separate, e.g. `--weight mul=4,beq=0`. 0
    /// excludes the opcode. Beats `--class-weight` and `--default-weight`.
    /// The conditional branches and `jalr` set the branch density, and need
    /// entanglement (see the README).
    #[arg(long = "weight", value_name = "OPCODE=WEIGHT", value_delimiter = ',', value_parser = parse_weight::<Opcode>)]
    pub opcode_weights: Vec<(Opcode, f64)>,

    /// Relative weight of every opcode in a class, as `CLASS=WEIGHT`, e.g.
    /// `--class-weight memory=3,branch=0.5`. Classes are the names in the
    /// README, case-insensitive.
    #[arg(long = "class-weight", value_name = "CLASS=WEIGHT", value_delimiter = ',', value_parser = parse_weight::<InstructionClass>)]
    pub class_weights: Vec<(InstructionClass, f64)>,

    /// Weight of every opcode not named by `--weight` or `--class-weight`.
    /// Branches and `jalr` get a quarter of it.
    #[arg(long, default_value = "1.0")]
    pub default_weight: f64,

    /// Seed for generation: the same seed, options, and weights always give
    /// the same program. Random by default. `many` uses seed + index.
    #[arg(long)]
    pub seed: Option<u64>,
}

/// The CLI's defaults, for library users that fill in only what they change.
impl Default for CommonOpts {
    fn default() -> Self {
        #[derive(Parser)]
        struct Defaults {
            #[command(flatten)]
            common: CommonOpts,
        }
        Defaults::parse_from(["rvgen"]).common
    }
}

impl CommonOpts {
    pub fn instr_weights(&self) -> InstrWeights {
        InstrWeights {
            default: self.default_weight,
            classes: self.class_weights.iter().copied().collect(),
            opcodes: self.opcode_weights.iter().copied().collect(),
        }
    }
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

fn parse_weight<T: FromStr>(value: &str) -> Result<(T, f64), String>
where
    T::Err: Display,
{
    let (name, weight) = value
        .split_once('=')
        .ok_or_else(|| format!("`{value}` is not NAME=WEIGHT"))?;
    let (name, weight) = (name.trim(), weight.trim());
    let key = name.parse::<T>().map_err(|error| format!("`{name}`: {error}"))?;
    let weight = weight
        .parse::<f64>()
        .map_err(|error| format!("invalid weight `{weight}`: {error}"))?;
    Ok((key, weight))
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
