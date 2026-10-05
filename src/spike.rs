//! Run generated programs on Spike, the reference ISA simulator.
//!
//! Spike computes the register state a correct core must reach, which the
//! self-check block then compares against on the device under test.
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use std::{env, fs, process, thread};

use anyhow::{Context, Result, anyhow, bail, ensure};

use crate::riscv::{Extension, XReg};
use crate::target::Target;

/// Wall-clock limit for one Spike run. Generated programs are straight-line
/// and short, so hitting this means Spike is stuck.
const TIMEOUT: Duration = Duration::from_secs(60);

/// Architectural register values of one hart. FP registers hold the low 64
/// bits of Spike's register file and are zero when the target has no FPU.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArchState {
    pub xregs: [u64; 32],
    pub fregs: [u64; 32],
}

pub struct Spike {
    path: String,
    isa: String,
    privileges: String,
    memory: String,
    pmp_regions: usize,
    has_fpu: bool,
}

impl Spike {
    pub fn new(path: &str, target: &Target) -> Self {
        let ram = &target.physical_memory;
        Self {
            path: path.into(),
            isa: target.spike_isa(),
            privileges: target.spike_priv(),
            memory: format!("-m{:#x}:{:#x}", ram.start, ram.size),
            pmp_regions: target.pmp_regions,
            has_fpu: target.has(Extension::F),
        }
    }

    /// Run hart 0 through `pcs`, stopping just before it executes each one,
    /// and return its registers at every stop. The PCs must be listed in the
    /// order they are reached. FP registers are reported at the last stop only.
    pub fn states_at(&self, elf: &[u8], pcs: &[u64]) -> Result<Vec<ArchState>> {
        let elf_file = TempFile::new("elf", elf)?;
        let mut commands = String::new();
        for pc in pcs {
            commands.push_str(&format!("until pc 0 {pc:#x}\nreg 0\n"));
        }
        if self.has_fpu {
            for index in 0..32 {
                commands.push_str(&format!("freg 0 {index}\n"));
            }
        }
        commands.push_str("q\n");
        let command_file = TempFile::new("cmds", commands.as_bytes())?;

        let (status, output) = self.run(&[
            "-d".into(),
            format!("--debug-cmd={}", command_file.0.display()),
            elf_file.0.display().to_string(),
        ])?;
        // Each `reg 0` dump starts with `zero:`; FP values follow the last one.
        let starts: Vec<_> = output.match_indices("zero:").map(|(index, _)| index).collect();
        let reached = starts.len();
        let unreached = || {
            format!(
                "Spike did not reach {:#x} (exit code {status}); \
                 the program likely trapped or exited early. Spike output:\n{output}",
                pcs.get(reached).or(pcs.last()).copied().unwrap_or_default()
            )
        };
        ensure!(reached == pcs.len(), unreached());
        let mut states = Vec::with_capacity(reached);
        for (index, start) in starts.iter().enumerate() {
            let last = index + 1 == reached;
            let end = starts.get(index + 1).copied().unwrap_or(output.len());
            states.push(
                parse_state(&output[*start..end], self.has_fpu && last)
                    .with_context(unreached)?,
            );
        }
        Ok(states)
    }

    /// Run to completion and return the HTIF exit code (0 means pass).
    pub fn exit_code(&self, elf: &[u8]) -> Result<i32> {
        let elf_file = TempFile::new("elf", elf)?;
        Ok(self.run(&[elf_file.0.display().to_string()])?.0)
    }

    /// Return the exit status and stderr, where Spike prints both debug
    /// output and HTIF failure messages.
    fn run(&self, args: &[String]) -> Result<(i32, String)> {
        let mut child = Command::new(&self.path)
            .arg(format!("--isa={}", self.isa))
            .arg(format!("--priv={}", self.privileges))
            .arg(&self.memory)
            // As many PMP entries as the device, so S and U see the same
            // permissions: with none, all memory is accessible.
            .arg(format!("--pmpregions={}", self.pmp_regions))
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("failed to launch Spike ({})", self.path))?;
        // Drain stderr concurrently so a full pipe cannot stall Spike.
        let mut stderr = child.stderr.take().expect("stderr is piped");
        let reader = thread::spawn(move || {
            let mut output = String::new();
            stderr.read_to_string(&mut output).map(|_| output)
        });

        let deadline = Instant::now() + TIMEOUT;
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                bail!("Spike timed out after {}s", TIMEOUT.as_secs());
            }
            thread::sleep(Duration::from_millis(5));
        };
        let output = reader
            .join()
            .map_err(|_| anyhow!("Spike output reader panicked"))??;
        let code = status
            .code()
            .ok_or_else(|| anyhow!("Spike was killed by a signal: {status}"))?;
        Ok((code, output))
    }
}

/// Parse `reg 0` (pairs such as `t0: 0x...`, by ABI name) followed by one
/// `freg 0 N` line per FP register (a bare hex value up to 128 bits wide).
fn parse_state(output: &str, has_fpu: bool) -> Result<ArchState> {
    let mut state = ArchState::default();
    let mut found = [false; 32];
    let mut tokens = output.split_whitespace();
    while let Some(token) = tokens.next() {
        let Some(name) = token.strip_suffix(':') else {
            continue;
        };
        let Some(index) = XReg::ABI_NAMES.iter().position(|abi| *abi == name) else {
            continue;
        };
        let value = tokens.next().unwrap_or_default();
        state.xregs[index] = parse_hex(value).with_context(|| format!("bad value for {name}"))? as u64;
        found[index] = true;
    }
    ensure!(found.iter().all(|f| *f), "no complete integer register dump");

    if has_fpu {
        let values: Vec<_> = output
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("0x") && !line.contains(char::is_whitespace))
            .collect();
        ensure!(
            values.len() == 32,
            "expected 32 FP register dumps, found {}",
            values.len()
        );
        for (index, value) in values.into_iter().enumerate() {
            state.fregs[index] = parse_hex(value)? as u64;
        }
    }
    Ok(state)
}

fn parse_hex(value: &str) -> Result<u128> {
    let digits = value
        .strip_prefix("0x")
        .ok_or_else(|| anyhow!("not a hex value: {value:?}"))?;
    Ok(u128::from_str_radix(digits, 16)?)
}

/// A uniquely named file in the system temp directory, removed on drop.
struct TempFile(PathBuf);

impl TempFile {
    fn new(extension: &str, contents: &[u8]) -> Result<Self> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!("rvgen-{}-{id}.{extension}", process::id()));
        fs::write(&path, contents).with_context(|| format!("cannot write {}", path.display()))?;
        Ok(Self(path))
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REG_DUMP: &str = "\
zero: 0x0000000000000000  ra: 0x29a0b83ebc479aca  sp: 0x480a2f77d639ff68  gp: 0xa47f24fe1e6a6cfe
  tp: 0x41a41d83a87f57ea  t0: 0x0000000000000000  t1: 0x0000000088000079  t2: 0xfffffffffffff9fe
  s0: 0xfb62b6aeccf4b947  s1: 0x02d91fb32c45faa5  a0: 0xffffffff7fc00000  a1: 0x0000000087fffa0d
  a2: 0xb929ae4c07a121df  a3: 0x00000000000087ff  a4: 0x0000000000000000  a5: 0x0000000000000000
  a6: 0x0000000000000000  a7: 0x0000000000000000  s2: 0xb6d8ce7b0a82c287  s3: 0x56881724acd9241e
  s4: 0x23b9b5d0050740e5  s5: 0xc39d22ab02c47b69  s6: 0x04ede188290e3a1a  s7: 0x39fdc9ca4f00f9c0
  s8: 0xf9ebf89362a825bc  s9: 0xea1940ff75874f57 s10: 0xc1f079287fee7999 s11: 0x00796484e01b1cfb
  t3: 0x00000000880002b5  t4: 0x000000000000001d  t5: 0x7fffffffffffffff  t6: 0x0000000001658000
";

    #[test]
    fn parses_integer_registers() {
        let state = parse_state(REG_DUMP, false).unwrap();
        assert_eq!(state.xregs[0], 0);
        assert_eq!(state.xregs[1], 0x29a0b83ebc479aca);
        assert_eq!(state.xregs[26], 0xc1f079287fee7999); // s10
        assert_eq!(state.xregs[31], 0x0000000001658000);
    }

    #[test]
    fn parses_fp_registers_as_low_64_bits() {
        let mut output = REG_DUMP.to_string();
        for index in 0..32u64 {
            output.push_str(&format!("0xffffffffffffffff{:016x}\n", 0xcf00_0000 + index));
        }
        let state = parse_state(&output, true).unwrap();
        assert_eq!(state.fregs[0], 0xcf00_0000);
        assert_eq!(state.fregs[31], 0xcf00_001f);
    }

    #[test]
    fn rejects_missing_dump() {
        assert!(parse_state("*** FAILED *** (tohost = 3)", false).is_err());
    }
}
