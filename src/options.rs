use crate::csrs::CsrRange;
use crate::privilege::{Cause, IllegalForm, TvecMode};
use crate::riscv::{Extension, InstructionClass, Opcode, PrivilegeLevel, Xlen};
use crate::weights::InstrWeights;
use clap::{Args, Parser};
use std::fmt::Display;
use std::str::FromStr;

#[derive(Args, Clone)]
pub struct CommonOpts {
    /// TOML file of options keyed by their long flag names, e.g.
    /// `isa = ["i", "m", "zicsr"]`, `num_instrs = 4096`, `misaligned_traps = true`.
    /// Flags given on the command line win.
    #[arg(long, value_name = "FILE")]
    pub config: Option<std::path::PathBuf>,

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

    // Code area size in bytes: the blocks are scattered over it and random
    // bytes fill the gaps. Drawn per program when omitted, log-uniformly
    // between four times the code (at least 16 KiB) and 1 MiB or the RAM left
    // below the data. A fixed size only needs to hold the code; a small one
    // keeps the image short for targets that load it slowly.
    #[arg(long, value_parser = parse_address)]
    pub code_area_size: Option<u64>,

    // Writable/executable SMC section size in bytes; zero omits the section.
    #[arg(long, default_value = "8192")]
    pub smc_size: u64,

    // Probability that a workload block is self-modifying: its code is
    // stored by earlier code, then made visible with `fence.i` (needs
    // zifencei; see doc/smc.md). 0 disables SMC.
    #[arg(long, default_value = "0.1")]
    pub smc_proba: f64,

    // PMP entries the target implements (at most 16); `_init` configures
    // them, never locked. 0 leaves PMP untouched.
    #[arg(long, default_value = "8")]
    pub pmp_regions: usize,

    // `medeleg` bits the device under test delegates (e.g. `0xb15d`); traps
    // from S and U are only routed to S through these (and those Spike
    // delegates too). 0 sends every trap to M.
    #[arg(long, default_value = "0", value_parser = parse_address)]
    pub medeleg_mask: u64,

    // The device traps on misaligned loads and stores, so the generator may
    // raise them on purpose.
    #[arg(long)]
    pub misaligned_traps: bool,

    // Self-modifying code is stored with naturally aligned stores only, for
    // a device that neither traps on nor supports misaligned stores.
    #[arg(long)]
    pub smc_aligned_stores: bool,

    // Trap vector modes `mtvec`/`stvec` implement: `direct`, `vectored` or
    // both. With one, every vector is written in that mode, the sink
    // handlers' included (Ibex is vectored only).
    #[arg(long, default_value = "direct,vectored", value_delimiter = ',')]
    pub tvec_modes: Vec<TvecMode>,

    // Alignment in bytes the target forces on a direct-mode trap vector's
    // BASE; landings armed in direct mode are aligned to it.
    #[arg(long, default_value = "4", value_parser = parse_address)]
    pub tvec_align: u64,

    // The same for a vectored-mode trap vector (Rocket and Ibex ignore
    // BASE's low 8 bits then).
    #[arg(long, default_value = "256", value_parser = parse_address)]
    pub vectored_tvec_align: u64,

    // Probability that a trap vector is armed in vectored mode, where
    // `--tvec-modes` allows both; drawn per program in 0..0.5 when omitted.
    #[arg(long)]
    pub vectored_proba: Option<f64>,

    // Probability that a block ends with a privilege switch (a descent or a
    // planned trap); drawn per program in 0..0.3 when omitted. 0 keeps the
    // hart in M with no planned trap.
    #[arg(long)]
    pub switch_proba: Option<f64>,

    // Probability that a block changes the privileged configuration (FPU
    // on/off, TSR, TVM, medeleg); drawn per program in 0..0.1 when omitted.
    #[arg(long)]
    pub config_proba: Option<f64>,

    /// Relative weight of a planned trap's cause, as `CAUSE=WEIGHT`, e.g.
    /// `--trap-weight illegal=4,breakpoint=0`. 0 never raises the cause;
    /// causes not named draw a weight per program. Causes: instr-misaligned,
    /// instr-access, illegal, breakpoint, load-misaligned, load-access,
    /// store-misaligned, store-access, ecall-u, ecall-s, ecall-m.
    #[arg(long = "trap-weight", value_name = "CAUSE=WEIGHT", value_delimiter = ',', value_parser = parse_weight::<Cause>)]
    pub trap_weights: Vec<(Cause, f64)>,

    /// CSRs the target does not implement, which no generated code then
    /// accesses: names (`mcountinhibit`, `mhpmevent7`; `mhpmevent`,
    /// `mhpmcounter` and `pmpaddr` name all of theirs), addresses (`0x320`)
    /// or address ranges (`0x323-0x33f`).
    #[arg(long, value_delimiter = ',')]
    pub disabled_csrs: Vec<CsrRange>,

    /// Instructions a planned illegal-instruction trap must not use, where
    /// the target does not trap on them as Spike does: `zero` (an all-zero
    /// word), `cycle-write` (a write to the read-only `cycle`), `mret`,
    /// `m-csr` (an M CSR read below M), `sret`, `s-csr` (an S CSR read in U),
    /// `tvm` (`sfence.vma`/`satp` in S with TVM), `fpu-off` (FP with the FPU off).
    #[arg(long, value_delimiter = ',')]
    pub disabled_illegal: Vec<IllegalForm>,

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
