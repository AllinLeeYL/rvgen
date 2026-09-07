use object::{Object, ObjectSection, ObjectSymbol};
use rvgen::{
    generator::GenerationAction, runtime::executable, Generator, GeneratorParams, InstructionClass,
};
use std::{
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[test]
fn executable_entry_mailboxes_and_symbols() {
    for is_64bit in [false, true] {
        let bytes = executable(&[], is_64bit, 0x8000_0000).unwrap();
        let file = object::File::parse(bytes.as_slice()).unwrap();
        assert_eq!(file.is_64(), is_64bit);
        let text = file.section_by_name(".text").unwrap();
        assert_eq!(text.address(), file.entry());
        assert_ne!(&text.data().unwrap()[..4], &[0; 4]);
        for name in ["tohost", "fromhost"] {
            let section = file.section_by_name(&format!(".{name}")).unwrap();
            let symbol = file.symbols().find(|s| s.name().unwrap() == name).unwrap();
            assert!(symbol.is_global());
            assert_eq!(symbol.address(), section.address());
            assert_eq!(symbol.size(), 8);
            assert_eq!(section.address() % 64, 0);
            assert_eq!(section.data().unwrap(), &[0; 8]);
            assert_eq!(section.flags(), object::SectionFlags::Elf { sh_flags: 3 });
        }
    }
    assert!(executable(&[], true, 0x8000_0002).is_err());
    assert!(executable(&[0], true, 0x8000_0000).is_err());
    assert!(executable(&[], false, 0xffff_fff0).is_err());
    assert!(executable(&[], true, u64::MAX - 3).is_err());
}

#[test]
fn spike_exits_on_success_and_traps_in_rv32_and_rv64() {
    let spike = std::env::var_os("SPIKE").unwrap_or_else(|| "spike".into());
    if Command::new(&spike).arg("--help").output().is_err() {
        eprintln!("Spike unavailable; skipping simulation test");
        return;
    }
    for is_64bit in [false, true] {
        let long_body = 0x0000_0013_u32.to_le_bytes().repeat(2048);
        let mut generator = Generator::new(GeneratorParams {
            size: 511,
            num_bbs: 7,
            seed: 1729,
            is_64bit,
            ..Default::default()
        })
        .unwrap();
        generator.choice_weights = vec![
            (InstructionClass::Alu.into(), 1.0),
            (InstructionClass::MulDiv.into(), 1.0),
            (InstructionClass::Branch.into(), 1.0),
            (InstructionClass::Jal.into(), 1.0),
            (InstructionClass::Fence.into(), 1.0),
            (GenerationAction::RegFsm.into(), 1.0),
        ];
        generator.generate().unwrap();
        let generated_body = generator.cores[0].get_bytecode();
        for (case, body, expected) in [
            ("empty", &[][..], 0),
            ("long", long_body.as_slice(), 0),
            ("compressed", &[1, 0][..], 0),
            ("illegal", &[0xff; 4][..], 1),
            ("generated", generated_body.as_slice(), 0),
        ] {
            let path = std::env::temp_dir().join(format!(
                "rvgen-spike-{}-{is_64bit}-{case}.elf",
                std::process::id()
            ));
            std::fs::write(&path, executable(body, is_64bit, 0x8000_0100).unwrap()).unwrap();
            let mut child = Command::new(&spike)
                .args([
                    "-m64",
                    if is_64bit {
                        "--isa=rv64gc"
                    } else {
                        "--isa=rv32gc"
                    },
                ])
                .arg(&path)
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let started = Instant::now();
            while child.try_wait().unwrap().is_none() && started.elapsed() < Duration::from_secs(5)
            {
                std::thread::sleep(Duration::from_millis(5));
            }
            if child.try_wait().unwrap().is_none() {
                child.kill().unwrap();
            }
            let output = child.wait_with_output().unwrap();
            let _ = std::fs::remove_file(&path);
            assert_eq!(
                output.status.code(),
                Some(expected),
                "{is_64bit} {case}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(!String::from_utf8_lossy(&output.stderr).contains("symbols not in ELF"));
        }
    }
}
