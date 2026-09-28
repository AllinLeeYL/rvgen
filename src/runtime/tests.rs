use super::memory::{MemoryRegion, Permissions, SectionRequest};
use super::*;
use crate::basicblock::BasicBlock;
use crate::riscv::{Csr, FReg, Instruction, RoundingMode};
use crate::utils::{SHF_EXECINSTR, SHF_WRITE};
use rand::{SeedableRng, rngs::StdRng};

fn runtime(xlen: Xlen, extensions: &[Extension], scratch: usize, smc: usize) -> Runtime {
    Runtime::new(
        Target::new(xlen, extensions.iter().copied()).unwrap(),
        MemoryPlan::bare_metal(0x8000_0000, 0x100000, scratch, smc).unwrap(),
        [],
    )
    .unwrap()
}

fn words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect()
}

#[test]
fn floating_point_startup_matches_extensions_and_xlen() {
    for xlen in [Xlen::X32, Xlen::X64] {
        for fp in [None, Some(Extension::F), Some(Extension::D)] {
            let mut extensions = vec![Extension::I, Extension::Zicsr];
            extensions.extend(fp);
            let rt = runtime(xlen, &extensions, 4096, 0);
            let code = RuntimeCode::assemble(&[], &rt).unwrap();
            let words = words(&code.bytes);
            assert_eq!(
                words.iter().filter(|w| **w & 0x7f == 0x53).count(),
                if fp.is_some() { 32 } else { 0 }
            );
            let clear_fcsr = Instruction::Csrrw {
                rd: XReg::ZERO,
                rs1: XReg::ZERO,
                csr: Csr::FCSR,
            }
            .encode()
            .unwrap()
            .bits();
            assert_eq!(words.contains(&clear_fcsr), fp.is_some());
            if let Some(extension) = fp {
                for index in 0..32 {
                    let rd = FReg::new(index).unwrap();
                    let expected = match (extension, xlen) {
                        (Extension::D, Xlen::X32) => Instruction::FcvtDW {
                            rd,
                            rs1: XReg::ZERO,
                            rm: RoundingMode::Rne,
                        },
                        (Extension::D, Xlen::X64) => Instruction::FmvDX {
                            rd,
                            rs1: XReg::ZERO,
                        },
                        _ => Instruction::FmvWX {
                            rd,
                            rs1: XReg::ZERO,
                        },
                    };
                    assert!(words.contains(&expected.encode_for(xlen).unwrap().bits()));
                }
            }
            let elf = rt.executable(&[]).unwrap();
            assert_eq!(elf[4], if xlen == Xlen::X64 { 2 } else { 1 });
        }
    }
}

#[test]
fn target_dependencies_and_runtime_requirements_are_explicit() {
    let target = Target::new(Xlen::X32, [Extension::I, Extension::D, Extension::D]).unwrap();
    assert!(target.has(Extension::F) && target.has(Extension::Zicsr));
    assert!(!target.supports(Opcode::FmvDX));
    assert!(!target.supports(Opcode::Mul));
    assert!(Target::new(Xlen::X64, [Extension::F]).is_err());
    let integer = Target::new(Xlen::X64, [Extension::I]).unwrap();
    assert!(
        Runtime::new(
            integer,
            MemoryPlan::bare_metal(0, 65536, 16, 0).unwrap(),
            []
        )
        .is_err()
    );
}

#[test]
fn startup_pointers_are_resolved_from_the_actual_sections() {
    for xlen in [Xlen::X32, Xlen::X64] {
        for scratch_size in [16, 96, 4096, 8192] {
            let rt = runtime(xlen, &[Extension::I, Extension::Zicsr], scratch_size, 1024);
            let mut code = RuntimeCode::assemble(&[], &rt).unwrap();
            let layout = rt.memory.resolve(code.bytes.len()).unwrap();
            code.resolve_addresses(&layout).unwrap();
            let words = words(&code.bytes);
            assert!(!words.contains(&0), "unresolved startup slot");
            for (register, section, offset) in [
                (
                    SCRATCH_BASE_REGISTER,
                    ".scratch",
                    rt.scratch_window().base_offset(),
                ),
                (SMC_BASE_REGISTER, ".smc", 0),
            ] {
                let index = words
                    .iter()
                    .position(|word| word & 0xfff == (u32::from(register.index()) << 7) | 0x17)
                    .unwrap();
                let pc = layout.get(".text").unwrap().address + index as u64 * 4;
                let high = (words[index] & 0xfffff000) as i32 as i64;
                let low = (words[index + 1] as i32 >> 20) as i64;
                assert_eq!(
                    pc.wrapping_add_signed(high + low),
                    layout.get(section).unwrap().address + offset
                );
            }
        }
    }
}

#[test]
fn layout_uses_permissions_and_named_symbols_instead_of_section_indices() {
    let base = 0x8000_0000;
    let requests = vec![
        SectionRequest::new(".smc", 512, 64, Permissions::RWX),
        SectionRequest::new(".extra", 32, 16, Permissions::RW),
        SectionRequest::new(".scratch", 128, 64, Permissions::RW),
        SectionRequest::new(".fromhost", 8, 64, Permissions::RW),
        SectionRequest::new(".tohost", 8, 64, Permissions::RW),
    ];
    let plan = MemoryPlan::new(
        vec![
            MemoryRegion {
                start: base,
                size: 4096,
                permissions: Permissions::RX,
            },
            MemoryRegion {
                start: base + 0x2000,
                size: 4096,
                permissions: Permissions::RW,
            },
            MemoryRegion {
                start: base + 0x4000,
                size: 4096,
                permissions: Permissions::RWX,
            },
        ],
        base,
        requests,
    )
    .unwrap();
    let rt = Runtime::new(
        Target::new(Xlen::X64, [Extension::I, Extension::Zicsr]).unwrap(),
        plan,
        [],
    )
    .unwrap();
    let mut code = RuntimeCode::assemble(&[], &rt).unwrap();
    let layout = rt.memory.resolve(code.bytes.len()).unwrap();
    assert_eq!(layout.get(".smc").unwrap().address, base + 0x4000);
    assert_eq!(layout.get(".extra").unwrap().address, base + 0x2000);
    code.resolve_addresses(&layout).unwrap();
    let symbols = code.symbols(&layout).unwrap();
    let sections = layout.sections(code.bytes).unwrap();
    for name in ["tohost", "fromhost"] {
        let symbol = symbols.iter().find(|s| s.name == name).unwrap();
        assert_eq!(sections[symbol.section].name, format!(".{name}"));
    }
    assert_eq!(
        sections[layout.index(".scratch").unwrap()].flags & SHF_EXECINSTR,
        0
    );
    assert_ne!(sections[layout.index(".smc").unwrap()].flags & SHF_WRITE, 0);
    rt.executable(&[]).unwrap();
}

#[test]
fn invalid_memory_plans_are_rejected() {
    let region = MemoryRegion {
        start: 0x80000000,
        size: 4096,
        permissions: Permissions::RWX,
    };
    assert!(MemoryPlan::new(vec![region.clone(), region.clone()], region.start, vec![]).is_err());
    assert!(MemoryPlan::new(vec![region.clone()], region.start + 2, vec![]).is_err());
    let request = SectionRequest::new(".bad", 8, 3, Permissions::RW);
    assert!(MemoryPlan::new(vec![region.clone()], region.start, vec![request]).is_err());
    let request = SectionRequest::new(".same", 8, 8, Permissions::RW);
    assert!(
        MemoryPlan::new(
            vec![region.clone()],
            region.start,
            vec![request.clone(), request]
        )
        .is_err()
    );
    assert!(MemoryPlan::bare_metal(u64::MAX - 3, 8, 16, 0).is_err());
    let too_high = MemoryPlan::bare_metal(0x1_0000_0000, 4096, 16, 0).unwrap();
    assert!(
        Runtime::new(
            Target::new(Xlen::X32, [Extension::I, Extension::Zicsr]).unwrap(),
            too_high,
            []
        )
        .is_err()
    );
    let tiny = MemoryPlan::bare_metal(region.start, 64, 16, 0).unwrap();
    assert!(tiny.resolve(128).is_err());
    let too_full = MemoryPlan::bare_metal(region.start, 512, 4096, 0).unwrap();
    assert!(too_full.resolve(128).is_err());
}

#[test]
fn compressed_padding_and_optional_smc_follow_configuration() {
    let plain = runtime(Xlen::X64, &[Extension::I, Extension::Zicsr], 16, 0);
    assert!(plain.executable(&[1, 0]).is_err());
    assert!(plain.executable(&[0]).is_err());
    let compressed = runtime(
        Xlen::X64,
        &[Extension::I, Extension::Zicsr, Extension::C],
        16,
        0,
    );
    compressed.executable(&[1, 0]).unwrap();
    let code = RuntimeCode::assemble(&[], &plain).unwrap();
    let layout = plain.memory.resolve(code.bytes.len()).unwrap();
    assert!(layout.get(".smc").is_err());
    assert!(
        !code
            .symbols(&layout)
            .unwrap()
            .iter()
            .any(|s| s.name == "rvgen_smc")
    );
}

#[test]
fn scratch_bounds_include_the_full_access_and_respect_encoding_limits() {
    for size in [16, 17, 64, 4096, 8192] {
        let rt = runtime(Xlen::X64, &[Extension::I, Extension::Zicsr], size, 0);
        let window = rt.scratch_window();
        for access in [1, 2, 4, 8] {
            for (min, max) in [(-2048, 2047), (0, 124), (0, 504)] {
                let (low, high) = window.offset_bounds(access, min, max).unwrap();
                assert!(low >= min && high <= max);
                assert_eq!(low % access, 0);
                assert_eq!(high % access, 0);
                assert!(window.base_offset() as i64 + low as i64 >= 0);
                assert!(window.base_offset() as i64 + high as i64 + access as i64 <= size as i64);
            }
        }
    }
}

#[test]
fn workload_selection_obeys_target_and_disabled_opcodes_without_hanging() {
    let target = Target::new(Xlen::X32, [Extension::I, Extension::Zicsr]).unwrap();
    let memory = MemoryPlan::bare_metal(0x80000000, 65536, 64, 0).unwrap();
    let disabled: Vec<_> = Opcode::ALL
        .iter()
        .copied()
        .filter(|op| *op != Opcode::Lw)
        .collect();
    let rt = Runtime::new(target.clone(), memory.clone(), disabled).unwrap();
    let mut block = BasicBlock::new(0, false, 100);
    block.run(&mut StdRng::seed_from_u64(42), &rt).unwrap();
    assert_eq!(block.instrs.len(), 200);
    for pair in block.instrs.chunks_exact(2) {
        assert!(matches!(
            pair[0],
            Instruction::Addi {
                rs1: SCRATCH_BASE_REGISTER,
                imm: 0,
                ..
            }
        ));
        let Instruction::Lw { imm, .. } = pair[1] else {
            panic!("unexpected opcode");
        };
        assert!((-32..=28).contains(&imm) && imm % 4 == 0);
    }
    block.encode(rt.target()).unwrap();
    let rt = Runtime::new(target, memory, Opcode::ALL.iter().copied()).unwrap();
    assert!(
        BasicBlock::new(0, false, 1)
            .run(&mut StdRng::seed_from_u64(42), &rt)
            .is_err()
    );
    rt.executable(&[]).unwrap(); // Workload exclusions do not disable runtime setup.
}

#[test]
fn far_memory_references_fail_instead_of_truncating_addresses() {
    let plan = MemoryPlan::new(
        vec![
            MemoryRegion {
                start: 0x80000000,
                size: 4096,
                permissions: Permissions::RX,
            },
            MemoryRegion {
                start: 0x2_00000000,
                size: 4096,
                permissions: Permissions::RW,
            },
        ],
        0x80000000,
        vec![
            SectionRequest::new(".tohost", 8, 64, Permissions::RW),
            SectionRequest::new(".fromhost", 8, 64, Permissions::RW),
            SectionRequest::new(".scratch", 64, 64, Permissions::RW),
        ],
    )
    .unwrap();
    let rt = Runtime::new(
        Target::new(Xlen::X64, [Extension::I, Extension::Zicsr]).unwrap(),
        plan,
        [],
    )
    .unwrap();
    assert!(
        rt.executable(&[])
            .unwrap_err()
            .to_string()
            .contains("AUIPC range")
    );
}
