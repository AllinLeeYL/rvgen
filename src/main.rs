use clap::Parser;
use rvgen::{cli::Cli, Generator};
use std::time::Instant;

fn main() {
    let cli = Cli::parse();
    let result = (|| -> rvgen::Result<()> {
        let options = cli.resolve()?;
        if options.params.num_cores != 1 {
            return Err("only one core is supported for ELF output".into());
        }
        let started = Instant::now();

        if options.out_dir.is_some() || options.num_elfs > 1 {
            let first = options.output_path(0);
            if let Some(parent) = first.parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent)?;
            }
        }

        /* Begin of Core Logic */
        for index in 0..options.num_elfs {
            let mut params = options.params.clone();
            params.seed += index as i64; // range validated before any files are written
            let mut generator = Generator::new(params)?;
            generator.generate()?;
            generator.gen_elf(options.output_path(index))?;
        }
        /* End of Core Logic */

        let elapsed = started.elapsed().as_secs_f64();
        eprintln!(
            "Generated {} ELF(s) in {:.6}s ({:.2} ELF/s)",
            options.num_elfs,
            elapsed,
            options.num_elfs as f64 / elapsed.max(f64::MIN_POSITIVE)
        );
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("rvgen: {error}");
        std::process::exit(1);
    }
}
