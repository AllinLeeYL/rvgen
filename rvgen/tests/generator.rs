use rvgen::{generator::Instruction, instgen::ISAInstrClass, Generator, GeneratorParams};

#[test]
fn default_output_and_assembly_match_python() {
    let mut generator = Generator::new(GeneratorParams::default()).unwrap();
    generator.generate().unwrap();
    assert_eq!(
        generator.elf().unwrap(),
        include_bytes!("fixtures/runtime64.bin")
    );
    assert_eq!(
        generator.assembly().unwrap(),
        include_str!("fixtures/assembly.S")
    );
    assert_eq!(generator.cores[0].bbs.len(), 12);
    assert!(generator.cores[0]
        .bbs
        .iter()
        .all(|bb| bb.selected_classes.len() == 256 && bb.insts.is_empty()));
}

#[test]
fn seed_reproduces_selection_and_zero_weights_are_never_selected() {
    let mut first = Generator::new(GeneratorParams {
        seed: 1729,
        ..Default::default()
    })
    .unwrap();
    let mut second = Generator::new(first.params.clone()).unwrap();
    first.generate().unwrap();
    second.generate().unwrap();
    let selections = |g: &Generator| {
        g.cores[0]
            .bbs
            .iter()
            .flat_map(|bb| bb.selected_classes.iter().copied())
            .collect::<Vec<_>>()
    };
    assert_eq!(selections(&first), selections(&second));
    for class in selections(&first) {
        assert!(first
            .inst_weights
            .iter()
            .any(|(c, w)| *c == class && *w > 0.0));
    }
    first.generate().unwrap();
    assert_eq!(selections(&first), selections(&second));
    second.params.seed += 1;
    second.generate().unwrap();
    assert_ne!(selections(&first), selections(&second));
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
    assert_eq!(
        single
            .inst_weights
            .iter()
            .find(|(c, _)| *c == ISAInstrClass::SEND_IPI)
            .unwrap()
            .1,
        0.0
    );
    assert!(
        multi
            .inst_weights
            .iter()
            .find(|(c, _)| *c == ISAInstrClass::SEND_IPI)
            .unwrap()
            .1
            > 0.0
    );
    multi.generate().unwrap();
    assert_eq!(multi.cores.len(), 2);
    assert!(multi.cores.iter().all(
        |core| core.bbs.len() == 3 && core.bbs.iter().all(|bb| bb.selected_classes.len() == 10)
    ));
    assert!(multi.elf().is_err());
}

#[test]
fn explicit_instructions_serialize_in_block_order() {
    let mut generator = Generator::new(GeneratorParams::default()).unwrap();
    generator.cores[0].bbs[0]
        .insts
        .push(Instruction::Standard(rvgen::riscv::rv32i_addi(1, 0, 42)));
    generator.cores[0].bbs[1]
        .insts
        .push(Instruction::Compressed(0x0001));
    assert_eq!(generator.cores[0].get_bytecode(), [0x93, 0, 0xa0, 2, 1, 0]);
    assert!(generator.assembly().unwrap().contains(".4byte 0x02a00093"));
    assert!(generator.assembly().unwrap().contains(".2byte 0x0001"));
}

#[test]
fn invalid_weights_return_errors_and_single_class_is_selected() {
    let mut generator = Generator::new(GeneratorParams::default()).unwrap();
    for weight in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        generator.inst_weights = vec![(ISAInstrClass::ALU, weight)];
        assert!(generator.generate().is_err());
    }
    generator.inst_weights.clear();
    assert!(generator.generate().is_err());
    generator.inst_weights = vec![(ISAInstrClass::ALU, 1.0)];
    generator.generate().unwrap();
    assert!(generator.cores[0]
        .bbs
        .iter()
        .flat_map(|bb| &bb.selected_classes)
        .all(|c| *c == ISAInstrClass::ALU));
}
