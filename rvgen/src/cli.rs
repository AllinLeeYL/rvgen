//! CLI arguments override the corresponding top-level TOML configuration keys.

use std::path::PathBuf;

use clap::Parser;
use serde::Deserialize;

use crate::{GeneratorParams, Result};

#[derive(Debug, Parser)]
#[command(version, about = "RISC-V instruction generator")]
pub struct Cli {
    /// Read defaults from a TOML file.
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Instruction selections per block (divided by the number of cores).
    #[arg(long)]
    pub size: Option<usize>,
    /// Reserved memory-size parameter; currently unused by the generator.
    #[arg(long)]
    pub memsize: Option<usize>,
    /// Number of cores; ELF output currently requires one core.
    #[arg(long)]
    pub num_cores: Option<usize>,
    /// Number of basic blocks per core.
    #[arg(long)]
    pub num_bbs: Option<usize>,
    /// Random seed (default: 0).
    #[arg(long, allow_hyphen_values = true)]
    pub seed: Option<i64>,
    /// Reserved privilege parameter; currently unused by the generator.
    #[arg(long, value_parser = parse_bool, action = clap::ArgAction::Set)]
    pub authorize_privileges: Option<bool>,
    /// Output ELF path (default: output.elf).
    #[arg(long, short = 'o')]
    pub out: Option<PathBuf>,
    /// Number of ELFs to generate in this process (default: 1).
    #[arg(long)]
    pub count: Option<usize>,
    /// Output directory, created if needed. Batch files use OUT's numbered basename.
    #[arg(long)]
    pub out_dir: Option<PathBuf>,
}

/// Matches the Python CLI: only 1, true, and yes (case insensitive) are true.
pub fn parse_bool(value: &str) -> std::result::Result<bool, String> {
    Ok(matches!(
        value.to_ascii_lowercase().as_str(),
        "1" | "true" | "yes"
    ))
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub size: Option<usize>,
    pub memsize: Option<usize>,
    pub num_cores: Option<usize>,
    pub num_bbs: Option<usize>,
    pub seed: Option<i64>,
    pub authorize_privileges: Option<bool>,
    pub out: Option<PathBuf>,
    pub count: Option<usize>,
    pub out_dir: Option<PathBuf>,
}

#[derive(Debug)]
pub struct Options {
    pub params: GeneratorParams,
    pub out: PathBuf,
    pub count: usize,
    pub out_dir: Option<PathBuf>,
}

impl Cli {
    pub fn resolve(self) -> Result<Options> {
        let config = match &self.config {
            Some(path) => {
                let source = std::fs::read_to_string(path)
                    .map_err(|error| format!("cannot read config {}: {error}", path.display()))?;
                toml::from_str(&source)
                    .map_err(|error| format!("invalid config {}: {error}", path.display()))?
            }
            None => Config::default(),
        };
        self.with_config(config)
    }

    pub fn with_config(self, config: Config) -> Result<Options> {
        let defaults = GeneratorParams::default();
        let params = GeneratorParams {
            size: self.size.or(config.size).unwrap_or(defaults.size),
            memsize: self.memsize.or(config.memsize).unwrap_or(defaults.memsize),
            num_cores: self
                .num_cores
                .or(config.num_cores)
                .unwrap_or(defaults.num_cores),
            num_bbs: self.num_bbs.or(config.num_bbs).unwrap_or(defaults.num_bbs),
            seed: self.seed.or(config.seed).unwrap_or(defaults.seed),
            authorize_privileges: self
                .authorize_privileges
                .or(config.authorize_privileges)
                .unwrap_or(defaults.authorize_privileges),
            ..defaults
        };
        params.validate()?;
        let count = self.count.or(config.count).unwrap_or(1);
        if count == 0 {
            return Err("count must be greater than zero".into());
        }
        params
            .seed
            .checked_add(i64::try_from(count - 1)?)
            .ok_or("batch seed range exceeds signed 64-bit integers")?;
        Ok(Options {
            params,
            out: self
                .out
                .or(config.out)
                .unwrap_or_else(|| "output.elf".into()),
            count,
            out_dir: self.out_dir.or(config.out_dir),
        })
    }
}

impl Options {
    pub fn output_path(&self, index: usize) -> PathBuf {
        let directory = self
            .out_dir
            .as_deref()
            .unwrap_or_else(|| self.out.parent().unwrap_or(std::path::Path::new(".")));
        if self.count == 1 {
            return directory.join(self.out.file_name().unwrap_or_default());
        }
        let mut name = self.out.file_stem().unwrap_or_default().to_os_string();
        name.push(format!("_{index:06}."));
        name.push(self.out.extension().unwrap_or(std::ffi::OsStr::new("elf")));
        directory.join(name)
    }
}
