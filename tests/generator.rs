use object::{Object, ObjectSection};
use rand::{rngs::StdRng, SeedableRng};
use rvgen::{
    generator::{
        BasicBlockGenerator, BasicBlockGeneratorParams, CoreGenerator, CoreGeneratorParams,
        GenerationAction as A, GenerationChoice as Choice, GenerationContext, SelectedChoice,
    },
    riscv::IntReg,
    Generator, GeneratorParams, Instruction, InstructionClass as C, InstructionKind as K,
};

fn selections(generator: &Generator) -> Vec<SelectedChoice> {
    generator
        .cores
        .iter()
        .flat_map(|core| &core.bbs)
        .flat_map(|bb| bb.selected_choices.iter().cloned())
        .collect()
}

fn instructions(generator: &Generator) -> Vec<Instruction> {
    generator
        .cores
        .iter()
        .flat_map(|core| &core.bbs)
        .flat_map(|bb| bb.insts.iter().copied())
        .collect()
}

fn assert_choice_ranges(generator: &Generator) {
    for block in generator.cores.iter().flat_map(|core| &core.bbs) {
        let mut end = 0;
        for selected in &block.selected_choices {
            assert_eq!(selected.instructions.start, end);
            let emitted = &block.insts[selected.instructions.clone()];
            match selected.choice {
                Choice::Instruction(class) => {
                    assert_eq!(emitted.len(), 1);
                    assert_eq!(emitted[0].class(), Some(class));
                }
                Choice::Action(A::Exception) => {
                    assert!(matches!(
                        emitted,
                        [Instruction::Ecall | Instruction::Ebreak]
                    ));
                }
                Choice::Action(A::RegFsm) => assert_eq!(emitted.len(), 2),
                Choice::Action(A::CreateAddressDependency) => {
                    let [Instruction::Addi {
                        rd: address,
                        rs1: source,
                        imm: 0,
                    }, Instruction::Lw {
                        rs1: consumer,
                        imm: 0,
                        ..
                    }] = emitted
                    else {
                        panic!("invalid dependency sequence: {emitted:?}");
                    };
                    assert_ne!(address, source);
                    assert_ne!(*address, IntReg::zero);
                    assert_eq!(address, consumer);
                }
                Choice::Action(A::FreePolluted) => assert!(emitted.is_empty()),
                choice => panic!("unexpected choice: {choice:?}"),
            }
            end = selected.instructions.end;
        }
        assert_eq!(end, block.insts.len());
        assert_eq!(block.insts.len(), block.params.num_insts);
    }
}

#[test]
fn default_generator_lowers_choices_and_renders_all_blocks() {
    let mut generator = Generator::new(GeneratorParams::default()).unwrap();
    generator.generate().unwrap();
    assert_eq!(instructions(&generator).len(), 256);
    let workload = generator.cores[0].get_bytecode();
    assert_eq!(workload.len(), 256 * 4);
    assert_choice_ranges(&generator);
    let assembly = generator.assembly().unwrap();
    assert!(!assembly.contains("{{"));
    for block in &generator.cores[0].bbs {
        assert_eq!(
            assembly
                .matches(&format!("\n{}:", block.params.label))
                .count(),
            1
        );
        for instruction in &block.insts {
            assert!(assembly.contains(&instruction.to_string()));
        }
    }
    assert_eq!(generator.cores[0].bbs.len(), 12);
    let bytes = generator.elf().unwrap();
    let file = object::File::parse(bytes.as_slice()).unwrap();
    let text = file.section_by_name(".text").unwrap().data().unwrap();
    assert_eq!(&text[12..12 + workload.len()], workload);
}

#[test]
fn seed_reproduces_choices_operands_and_output_without_accumulation() {
    let mut first = Generator::new(GeneratorParams {
        seed: 1729,
        ..Default::default()
    })
    .unwrap();
    let mut second = Generator::new(first.params.clone()).unwrap();
    first.generate().unwrap();
    second.generate().unwrap();
    assert_eq!(selections(&first), selections(&second));
    assert_eq!(instructions(&first), instructions(&second));
    assert_eq!(first.elf().unwrap(), second.elf().unwrap());
    for selected in selections(&first) {
        assert!(first
            .choice_weights
            .iter()
            .any(|(c, w)| *c == selected.choice && *w > 0.0));
    }
    first.generate().unwrap();
    assert_eq!(selections(&first), selections(&second));
    assert_eq!(instructions(&first), instructions(&second));
    second.params.seed += 1;
    second.generate().unwrap();
    assert_ne!(instructions(&first), instructions(&second));
    assert_ne!(first.elf().unwrap(), second.elf().unwrap());
    // Retain the existing signed seed convention, including i64::MIN.
    second.params.seed = -first.params.seed;
    second.generate().unwrap();
    assert_eq!(instructions(&first), instructions(&second));
    second.params.seed = i64::MIN;
    second.generate().unwrap();
}

#[test]
fn budgets_are_distributed_without_multiplication_or_lost_remainders() {
    for size in [0, 1, 2, 7, 21, 256] {
        for num_cores in [1, 2, 4] {
            for num_bbs in [1, 3, 12] {
                let mut generator = Generator::new(GeneratorParams {
                    size,
                    num_cores,
                    num_bbs,
                    ..Default::default()
                })
                .unwrap();
                generator.generate().unwrap();
                assert_eq!(instructions(&generator).len(), size);
                assert_eq!(generator.cores.len(), num_cores);
                assert_eq!(
                    generator
                        .cores
                        .iter()
                        .map(|c| c.params.num_insts)
                        .sum::<usize>(),
                    size
                );
                for core in &generator.cores {
                    assert_eq!(core.bbs.len(), num_bbs);
                    assert_eq!(
                        core.bbs.iter().map(|b| b.params.num_insts).sum::<usize>(),
                        core.params.num_insts
                    );
                    let counts: Vec<_> = core.bbs.iter().map(|b| b.insts.len()).collect();
                    assert!(counts.iter().max().unwrap() - counts.iter().min().unwrap() <= 1);
                }
                assert_choice_ranges(&generator);
            }
        }
    }
    let mut core = CoreGenerator::new(CoreGeneratorParams {
        num_bbs: 3,
        num_insts: 11,
    });
    core.generate(
        &mut StdRng::seed_from_u64(1),
        &[(C::Alu.into(), 1.0)],
        &GenerationContext::default(),
    )
    .unwrap();
    assert_eq!(
        core.bbs.iter().map(|bb| bb.insts.len()).collect::<Vec<_>>(),
        [4, 4, 3]
    );
}

#[test]
fn weights_are_local_and_multicore_output_is_rejected() {
    let single = Generator::new(GeneratorParams::default()).unwrap();
    let mut multi = Generator::new(GeneratorParams {
        num_cores: 2,
        size: 21,
        num_bbs: 3,
        ..Default::default()
    })
    .unwrap();
    let original = single.choice_weights.clone();
    multi.choice_weights = vec![(C::Alu.into(), 1.0)];
    multi.generate().unwrap();
    assert_eq!(single.choice_weights, original);
    assert_eq!(
        multi
            .cores
            .iter()
            .map(|c| c.params.num_insts)
            .collect::<Vec<_>>(),
        [11, 10]
    );
    assert_eq!(instructions(&multi).len(), 21);
    assert!(multi.elf().is_err());
}

#[test]
fn regeneration_uses_current_size_and_block_counts() {
    let mut generator = Generator::new(GeneratorParams::default()).unwrap();
    generator.generate().unwrap();
    generator.params.size = 7;
    generator.params.num_cores = 2;
    generator.params.num_bbs = 2;
    generator.generate().unwrap();
    assert_eq!(instructions(&generator).len(), 7);
    assert_eq!(generator.cores.len(), 2);
    assert!(generator.cores.iter().all(|core| core.bbs.len() == 2));
    assert_choice_ranges(&generator);
    generator.params.size = 0;
    generator.generate().unwrap();
    assert!(instructions(&generator).is_empty());
    assert!(selections(&generator).is_empty());
}

#[test]
fn explicit_instructions_serialize_in_block_order() {
    let mut generator = Generator::new(GeneratorParams::default()).unwrap();
    generator.cores[0].bbs[0].insts.push(Instruction::Addi {
        rd: IntReg::ra,
        rs1: IntReg::zero,
        imm: 42,
    });
    generator.cores[0].bbs[1].insts.push(Instruction::CAddi {
        rd: IntReg::zero,
        imm: 0,
    });
    assert_eq!(generator.cores[0].get_bytecode(), [0x93, 0, 0xa0, 2, 1, 0]);
    assert!(generator.assembly().unwrap().contains(".4byte 0x02a00093"));
    assert!(generator.assembly().unwrap().contains(".2byte 0x0001"));
    let bytes = generator.elf().unwrap();
    let file = object::File::parse(bytes.as_slice()).unwrap();
    let text = file.section_by_name(".text").unwrap().data().unwrap();
    assert_eq!(&text[12..18], &[0x93, 0, 0xa0, 2, 1, 0]);
}

#[test]
fn invalid_and_unsupported_policies_fail_without_replacing_the_workload() {
    let mut generator = Generator::new(GeneratorParams::default()).unwrap();
    generator.generate().unwrap();
    let original = instructions(&generator);
    let original_selections = selections(&generator);
    for weight in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        generator.choice_weights = vec![(C::Alu.into(), weight)];
        assert!(generator.generate().is_err());
    }
    generator.choice_weights.clear();
    assert!(generator.generate().is_err());
    generator.choice_weights = vec![(C::Alu.into(), f64::MAX), (C::Branch.into(), f64::MAX)];
    assert!(generator.generate().is_err());
    for action in [
        A::SendIpi,
        A::DescendPrivilege,
        A::WriteInstruction,
        A::WaitForInstruction,
    ] {
        generator.choice_weights = vec![(action.into(), 1.0)];
        assert!(generator
            .generate()
            .unwrap_err()
            .to_string()
            .contains("not implemented"));
    }
    assert_eq!(instructions(&generator), original);
    assert_eq!(selections(&generator), original_selections);
    generator.choice_weights = vec![(C::Alu.into(), 1.0), (A::SendIpi.into(), 0.0)];
    generator.generate().unwrap();
    assert!(selections(&generator)
        .iter()
        .all(|s| s.choice == C::Alu.into()));
}

#[test]
fn action_sequences_stay_whole_and_no_op_policies_terminate() {
    for action in [A::RegFsm, A::CreateAddressDependency, A::Exception] {
        let mut generator = Generator::new(GeneratorParams {
            size: 8,
            num_bbs: 1,
            ..Default::default()
        })
        .unwrap();
        generator.choice_weights = vec![(action.into(), 1.0)];
        generator.generate().unwrap();
        assert_choice_ranges(&generator);
        assert!(selections(&generator)
            .iter()
            .all(|s| s.choice == action.into()));
    }
    let mut block = BasicBlockGenerator::new(BasicBlockGeneratorParams {
        num_insts: 3,
        label: "sequence".into(),
    });
    let context = GenerationContext::default();
    let mut rng = StdRng::seed_from_u64(3);
    // A final half-sequence is an error, and no partial workload is committed.
    assert!(block
        .generate(&mut rng, &[(A::RegFsm.into(), 1.0)], &context)
        .is_err());
    assert!(block.insts.is_empty());
    assert!(block.selected_choices.is_empty());
    assert!(block
        .generate(&mut rng, &[(A::FreePolluted.into(), 1.0)], &context)
        .is_err());
    // A tiny positive instruction weight still makes progress after a no-op.
    block
        .generate(
            &mut rng,
            &[
                (A::FreePolluted.into(), 1.0),
                (C::Alu.into(), f64::MIN_POSITIVE),
            ],
            &context,
        )
        .unwrap();
    assert_eq!(block.insts.len(), 3);
    assert!(block
        .selected_choices
        .iter()
        .any(|s| s.choice == A::FreePolluted.into() && s.instructions.is_empty()));
    // A two-instruction action cannot fit; the remaining instruction class can.
    block.params.num_insts = 1;
    block
        .generate(
            &mut rng,
            &[(A::RegFsm.into(), 1.0), (C::Alu.into(), f64::MIN_POSITIVE)],
            &context,
        )
        .unwrap();
    assert_eq!(block.insts.len(), 1);
    assert_eq!(block.insts[0].class(), Some(C::Alu));
}

#[test]
fn every_sampled_family_uses_table_kinds_and_encodable_operands() {
    use std::collections::HashSet;
    let classes: HashSet<_> = K::ALL.iter().filter_map(|kind| kind.class()).collect();
    assert_eq!(classes.len(), 19);
    // Iterate in declaration order so the test also has deterministic RNG use.
    let mut seen_classes = HashSet::new();
    let mut rng = StdRng::seed_from_u64(0x51a);
    for class in K::ALL
        .iter()
        .filter_map(|kind| kind.class())
        .filter(|class| seen_classes.insert(*class))
    {
        let mut kinds_seen = HashSet::new();
        for _ in 0..2048 {
            let instruction = class
                .lower(&mut rng, &GenerationContext::default())
                .unwrap();
            assert_eq!(instruction.class(), Some(class));
            assert_eq!(instruction.encode().byte_len(), 4);
            assert_eq!(instruction.encode().bits() & 3, 3);
            kinds_seen.insert(instruction.kind().unwrap());
        }
        assert_eq!(
            kinds_seen,
            class.kinds().collect(),
            "incomplete opcode sampling: {class:?}"
        );
    }
}

#[test]
fn rv32_and_privilege_constraints_are_applied_before_sampling() {
    let mut generator = Generator::new(GeneratorParams {
        is_64bit: false,
        authorize_privileges: false,
        ..Default::default()
    })
    .unwrap();
    generator.generate().unwrap();
    assert!(instructions(&generator)
        .iter()
        .all(|i| !i.class().is_some_and(C::requires_rv64) && i.kind() != Some(K::Addiw)));
    for class in [
        C::Alu64,
        C::MulDiv64,
        C::Memory64,
        C::Amo64,
        C::Float64,
        C::Double64,
        C::Csr,
    ] {
        generator.choice_weights = vec![(class.into(), 1.0)];
        assert!(generator.generate().is_err(), "{class:?}");
    }
    let mut rng = StdRng::seed_from_u64(8);
    for is_64bit in [false, true] {
        let context = GenerationContext {
            is_64bit,
            authorize_privileges: true,
        };
        for class in [C::Alu, C::Alu64]
            .into_iter()
            .filter(|c| is_64bit || !c.requires_rv64())
        {
            for _ in 0..1024 {
                let instruction = class.lower(&mut rng, &context).unwrap();
                match instruction {
                    Instruction::Slli { shamt, .. }
                    | Instruction::Srli { shamt, .. }
                    | Instruction::Srai { shamt, .. } => {
                        assert!((0..if is_64bit { 64 } else { 32 }).contains(&shamt));
                    }
                    Instruction::Slliw { shamt, .. }
                    | Instruction::Srliw { shamt, .. }
                    | Instruction::Sraiw { shamt, .. } => {
                        assert!((0..32).contains(&shamt));
                    }
                    _ => {}
                }
            }
        }
    }
}
