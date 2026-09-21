mod options;
mod basicblock;
mod orchestrator;
mod riscv;
mod utils;
mod context;
mod runtime;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "rvgen", about = "RISC-V program generator")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    One(options::OneOpts),
    Many(options::ManyOpts),
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::One(opts) => orchestrator::gen_one(opts, true)?,
        Command::Many(opts) => orchestrator::gen_many(opts)?,
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}
