use rvgen::{
    riscv::{rvwmo::RegType, FloatReg as F, IntReg as X},
    EncodedInstruction, Instruction as I, InstructionClass, InstructionKind,
};

#[test]
fn encodes_integer_float_atomic_csr_and_compressed_instructions() {
    // Independent reference words checked with GNU RISC-V as/ld/objdump,
    // -march=rv64gc, .option norvc for 32-bit instructions, and --no-relax.
    // Branch and JAL operands are relative to the instruction's PC.
    let cases = [
        (
            I::Addi {
                rd: X::ra,
                rs1: X::zero,
                imm: -1,
            },
            0xfff00093,
        ),
        (
            I::Add {
                rd: X::t0,
                rs1: X::t1,
                rs2: X::t2,
            },
            0x007302b3,
        ),
        (
            I::Sub {
                rd: X::t0,
                rs1: X::t1,
                rs2: X::t2,
            },
            0x407302b3,
        ),
        (
            I::Sw {
                rs1: X::t1,
                rs2: X::t2,
                imm: -16,
            },
            0xfe732823,
        ),
        (
            I::Beq {
                rs1: X::ra,
                rs2: X::sp,
                imm: -4,
            },
            0xfe208ee3,
        ),
        (
            I::Lui {
                rd: X::t0,
                imm: 0x12345,
            },
            0x123452b7,
        ),
        (I::Jal { rd: X::ra, imm: 8 }, 0x008000ef),
        (
            I::Ld {
                rd: X::t0,
                rs1: X::t1,
                imm: 24,
            },
            0x01833283,
        ),
        (
            I::Addw {
                rd: X::t0,
                rs1: X::t1,
                rs2: X::t2,
            },
            0x007302bb,
        ),
        (
            I::Mul {
                rd: X::t0,
                rs1: X::t1,
                rs2: X::t2,
            },
            0x027302b3,
        ),
        (
            I::Mulw {
                rd: X::t0,
                rs1: X::t1,
                rs2: X::t2,
            },
            0x027302bb,
        ),
        (
            I::LrW {
                rd: X::t0,
                rs1: X::t1,
                aq: true,
                rl: false,
            },
            0x140322af,
        ),
        (
            I::ScD {
                rd: X::t0,
                rs1: X::t1,
                rs2: X::t2,
                aq: false,
                rl: true,
            },
            0x1a7332af,
        ),
        (
            I::AmoaddD {
                rd: X::t0,
                rs1: X::t1,
                rs2: X::t2,
                aq: true,
                rl: true,
            },
            0x067332af,
        ),
        (
            I::Flw {
                rd: F::ft5,
                rs1: X::t1,
                imm: 12,
            },
            0x00c32287,
        ),
        (
            I::Fsd {
                rs1: X::t1,
                rs2: F::ft7,
                imm: 16,
            },
            0x00733827,
        ),
        (
            I::FmaddS {
                rd: F::ft5,
                rs1: F::ft6,
                rs2: F::ft7,
                rs3: F::fs0,
                rm: 0,
            },
            0x407302c3,
        ),
        (
            I::FaddD {
                rd: F::ft5,
                rs1: F::ft6,
                rs2: F::ft7,
                rm: 1,
            },
            0x027312d3,
        ),
        (
            I::FcvtLS {
                rd: X::t0,
                rs1: F::ft6,
                rm: 0,
            },
            0xc02302d3,
        ),
        (
            I::FcvtDL {
                rd: F::ft5,
                rs1: X::t1,
                rm: 0,
            },
            0xd22302d3,
        ),
        (
            I::FmvXD {
                rd: X::t0,
                rs1: F::ft6,
            },
            0xe20302d3,
        ),
        (
            I::Csrrw {
                rd: X::t0,
                rs1: X::t1,
                csr: 0x300,
            },
            0x300312f3,
        ),
        (
            I::Csrrwi {
                rd: X::t0,
                uimm: 7,
                csr: 0x300,
            },
            0x3003d2f3,
        ),
        (I::Fence { imm: 0x33 }, 0x0330000f),
        (I::FenceI { imm: 0 }, 0x0000100f),
        (I::Mret, 0x30200073),
        (I::CAddi { rd: X::ra, imm: 1 }, 0x0085),
        (I::CAddiw { rd: X::ra, imm: 1 }, 0x2085),
        (
            I::CMv {
                rd: X::t0,
                rs2: X::t1,
            },
            0x829a,
        ),
    ];
    for (instruction, expected) in cases {
        assert_eq!(instruction.encode().bits(), expected, "{instruction:?}");
        let width = if expected & 3 == 3 { 4 } else { 2 };
        assert_eq!(instruction.byte_len(), width);
        assert_eq!(instruction.encode().byte_len(), width);
    }
}

#[test]
fn mutations_update_encoding_without_changing_other_copies() {
    let mut instruction = I::Addi {
        rd: X::ra,
        rs1: X::zero,
        imm: 1,
    };
    let original = instruction;
    assert_eq!(original.encode().bits(), 0x00100093);
    if let I::Addi { rd, imm, .. } = &mut instruction {
        *rd = X::sp;
        *imm = -1;
    }
    assert_eq!(instruction.encode().bits(), 0xfff00113);
    assert_eq!(instruction.to_string(), ".4byte 0xfff00113");
    assert_eq!(original.encode().bits(), 0x00100093);
}

#[test]
fn ordering_mutations_preserve_atomic_opcode_and_operands() {
    for (aq, rl, expected) in [
        (false, false, 0x007332af),
        (false, true, 0x027332af),
        (true, false, 0x047332af),
        (true, true, 0x067332af),
    ] {
        let instruction = I::AmoaddD {
            rd: X::t0,
            rs1: X::t1,
            rs2: X::t2,
            aq,
            rl,
        };
        assert_eq!(instruction.encode().bits(), expected);
        assert_eq!(instruction.mnemonic(), Some("amoadd.d"));
    }
}

#[test]
fn mixed_width_and_raw_instructions_preserve_exact_bytes() {
    let instructions = [
        I::CAddi {
            rd: X::zero,
            imm: 0,
        },
        I::Addi {
            rd: X::ra,
            rs1: X::zero,
            imm: -1,
        },
        I::Standard(0xffff_ffff),
        I::Compressed(0),
    ];
    let mut bytes = Vec::new();
    for instruction in instructions {
        instruction.append_bytes(&mut bytes);
    }
    assert_eq!(
        bytes,
        [1, 0, 0x93, 0, 0xf0, 0xff, 0xff, 0xff, 0xff, 0xff, 0, 0]
    );
    assert_eq!(I::Compressed(0).to_string(), ".2byte 0x0000");
    for encoded in [
        EncodedInstruction::Standard(0xffff_ffff),
        EncodedInstruction::Compressed(0),
    ] {
        assert_eq!(I::from(encoded).encode(), encoded);
        assert!(I::from(encoded).mnemonic().is_none());
        assert!(I::from(encoded).kind().is_none());
        assert!(I::from(encoded).class().is_none());
        assert!(I::from(encoded).syntactic_dependency().is_none());
    }
}

#[test]
fn instruction_metadata_uses_canonical_mnemonics() {
    let store = I::Sw {
        rs1: X::t1,
        rs2: X::t2,
        imm: 0,
    };
    assert_eq!(store.mnemonic(), Some("sw"));
    assert_eq!(store.kind(), Some(InstructionKind::Sw));
    assert_eq!(store.class(), Some(InstructionClass::Memory));
    assert_eq!(
        store.syntactic_dependency().unwrap().sources,
        Some([RegType::Address, RegType::Data].as_slice())
    );
    let float = I::FdivD {
        rd: F::ft0,
        rs1: F::ft1,
        rs2: F::ft2,
        rm: 0,
    };
    assert_eq!(float.mnemonic(), Some("fdiv.d"));
    assert_eq!(float.kind(), Some(InstructionKind::FdivD));
    assert_eq!(float.class(), Some(InstructionClass::Double));
    assert!(float.syntactic_dependency().is_some());
    assert_eq!(I::CAddi { rd: X::ra, imm: 1 }.mnemonic(), Some("c.addi"));
}

#[test]
#[should_panic(expected = "compressed encoding exceeds 16 bits")]
fn compressed_encoding_is_never_silently_truncated() {
    // x0 is outside C.AND's x8..x15 register subset. The legacy helper's signed
    // bit operations produce upper bits; the new emission path must reject them.
    I::CAnd {
        rd: X::zero,
        rs2: X::zero,
    }
    .encode();
}
