use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

use clap::Parser;
use rvgen::{
    cli::{Cli, Config},
    GeneratorParams,
};

static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);
struct TempDir(PathBuf);
impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rvgen-test-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_rvgen"));
        command.current_dir(&self.0);
        command
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn defaults_and_cli_overrides() {
    let options = Cli::try_parse_from(["rvgen"]).unwrap().resolve().unwrap();
    assert_eq!(options.params, GeneratorParams::default());
    assert_eq!(options.out, PathBuf::from("output.elf"));
    let config: Config = toml::from_str("size = 10\nmemsize = 8192\nnum_cores = 2\nnum_bbs = 3\nseed = 7\nauthorize_privileges = true\nout = 'configured.elf'").unwrap();
    let options = Cli::try_parse_from([
        "rvgen",
        "--size",
        "17",
        "--num-cores=1",
        "--seed",
        "-42",
        "--authorize-privileges",
        "false",
        "-o",
        "explicit.elf",
    ])
    .unwrap()
    .with_config(config)
    .unwrap();
    assert_eq!(options.params.size, 17);
    assert_eq!(options.params.num_cores, 1);
    assert_eq!(options.params.num_bbs, 3);
    assert_eq!(options.params.memsize, 8192);
    assert_eq!(options.params.seed, -42);
    assert!(!options.params.authorize_privileges);
    assert_eq!(options.out, PathBuf::from("explicit.elf"));
}

#[test]
fn invalid_config_and_numeric_inputs_are_rejected() {
    for source in [
        "unknown = 1",
        "size = '12'",
        "num_cores = -1",
        "[instruction_weights]\nALU = 0.1",
        "seed = true",
    ] {
        assert!(toml::from_str::<Config>(source).is_err(), "{source}");
    }
    for args in [
        ["rvgen", "--size", "-1"],
        ["rvgen", "--size", "abc"],
        ["rvgen", "--unknown", "1"],
    ] {
        assert!(Cli::try_parse_from(args).is_err());
    }
    for flag in ["--num-cores", "--num-bbs"] {
        assert!(Cli::try_parse_from(["rvgen", flag, "0"])
            .unwrap()
            .resolve()
            .is_err());
    }
}

#[test]
fn binary_reads_config_and_runs_outside_repository() {
    let dir = TempDir::new();
    std::fs::write(
        dir.0.join("config.toml"),
        "size = 1\nnum_bbs = 2\nout = 'from config.elf'",
    )
    .unwrap();
    let output = dir
        .command()
        .args(["--config=config.toml", "--size", "0"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        std::fs::read(dir.0.join("from config.elf")).unwrap(),
        include_bytes!("fixtures/runtime64.bin")
    );
    assert!(!dir.0.join("output.elf").exists());
}

#[test]
fn errors_do_not_overwrite_output_and_help_needs_no_config() {
    let dir = TempDir::new();
    std::fs::write(dir.0.join("output.elf"), b"keep existing file").unwrap();
    for args in [
        vec!["--num-cores", "2"],
        vec!["--config", "missing.toml"],
        vec!["--out", "absent/output.elf"],
    ] {
        let output = dir.command().args(args).output().unwrap();
        assert!(!output.status.success());
        assert!(!output.stderr.is_empty());
        assert_eq!(
            std::fs::read(dir.0.join("output.elf")).unwrap(),
            b"keep existing file"
        );
    }
    let output = dir
        .command()
        .args(["--config", "missing.toml", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("--num-bbs"));
}

#[test]
fn batch_output_names_config_overrides_and_seed_range() {
    let dir = TempDir::new();
    std::fs::write(
        dir.0.join("batch.toml"),
        "count = 2\nout_dir = 'wrong'\nout = 'case.elf'\nseed = 11\nsize = 0",
    )
    .unwrap();
    let output = dir
        .command()
        .args([
            "--config",
            "batch.toml",
            "--count",
            "3",
            "--out-dir",
            "nested/output",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("Generated 3 ELF(s)"));
    assert!(!dir.0.join("wrong").exists());
    assert_eq!(
        std::fs::read_dir(dir.0.join("nested/output"))
            .unwrap()
            .count(),
        3
    );
    for index in 0..3 {
        assert_eq!(
            std::fs::read(dir.0.join(format!("nested/output/case_{index:06}.elf"))).unwrap(),
            include_bytes!("fixtures/runtime64.bin")
        );
    }
    for args in [
        vec!["--count", "0"],
        vec!["--count", "2", "--seed", "9223372036854775807"],
    ] {
        assert!(!dir.command().args(args).output().unwrap().status.success());
    }
    let cli = Cli::try_parse_from(["rvgen", "--count", "3", "--seed", "-1", "-o", "prefix/case"])
        .unwrap()
        .resolve()
        .unwrap();
    assert_eq!(cli.output_path(2), PathBuf::from("prefix/case_000002.elf"));
    assert_eq!(cli.params.seed, -1);
    let cli = Cli::try_parse_from(["rvgen", "--out-dir", "batch", "-o", "case.elf"])
        .unwrap()
        .resolve()
        .unwrap();
    assert_eq!(cli.output_path(0), PathBuf::from("batch/case.elf"));
}
