use rvgen::{options, orchestrator};

use anyhow::{Context, Result, bail};
use clap::parser::ValueSource;
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use std::ffi::OsString;
use std::path::PathBuf;

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

/// Parses the command line, splicing in `--config`'s options for the flags
/// not given on it, so clap validates and parses both the same way.
fn parse_cli(mut args: Vec<OsString>) -> Result<Cli> {
    let cli = Cli::command();
    let matches = cli.clone().get_matches_from(&args);
    let (name, sub) = matches.subcommand().expect("a subcommand is required");
    let Some(path) = sub.get_one::<PathBuf>("config") else {
        return Ok(Cli::from_arg_matches(&matches)?);
    };
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let table: toml::Table = toml::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    let subcommand = cli.find_subcommand(name).expect("matched subcommand");

    let mut extra = Vec::new();
    for (key, value) in table {
        let flag = key.replace('_', "-");
        let on_command_line = subcommand
            .get_arguments()
            .find(|arg| arg.get_long() == Some(flag.as_str()))
            .is_some_and(|arg| sub.value_source(arg.get_id().as_str()) == Some(ValueSource::CommandLine));
        if on_command_line {
            continue;
        }
        let scalar = |value: &toml::Value| match value {
            toml::Value::String(text) => Ok(text.clone()),
            toml::Value::Integer(number) => Ok(number.to_string()),
            toml::Value::Float(number) => Ok(number.to_string()),
            _ => bail!("{}: `{key}` must be a string, number, boolean or array of those", path.display()),
        };
        match &value {
            toml::Value::Boolean(true) => extra.push(format!("--{flag}")),
            toml::Value::Boolean(false) => {}
            toml::Value::Array(items) => {
                let items: Vec<String> = items.iter().map(scalar).collect::<Result<_>>()?;
                extra.push(format!("--{flag}={}", items.join(",")));
            }
            value => extra.push(format!("--{flag}={}", scalar(value)?)),
        }
    }
    // Right after the subcommand name; an unknown key fails as an unknown flag.
    let at = args.iter().position(|arg| arg == name).expect("matched subcommand") + 1;
    args.splice(at..at, extra.into_iter().map(OsString::from));
    Ok(Cli::parse_from(args))
}

fn run() -> Result<()> {
    let cli = parse_cli(std::env::args_os().collect())?;
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
