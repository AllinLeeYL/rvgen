use super::*;
use rand::{SeedableRng, rngs::StdRng};
use std::collections::HashSet;

pub(super) trait TestOperand {
    fn from_value(value: i64) -> Self;
}
macro_rules! numeric_test_operand {
    ($($ty:ty),*) => { $(impl TestOperand for $ty {
        fn from_value(value: i64) -> Self { value.try_into().unwrap() }
    })* };
}
numeric_test_operand!(i32, u8, u16);
impl TestOperand for XReg {
    fn from_value(value: i64) -> Self {
        Self::new(value.try_into().unwrap()).unwrap()
    }
}
impl TestOperand for FReg {
    fn from_value(value: i64) -> Self {
        Self::new(value.try_into().unwrap()).unwrap()
    }
}
impl TestOperand for Csr {
    fn from_value(value: i64) -> Self {
        Self::new(value.try_into().unwrap()).unwrap()
    }
}
impl TestOperand for RoundingMode {
    fn from_value(value: i64) -> Self {
        Self::try_from(u8::try_from(value).unwrap()).unwrap()
    }
}
impl TestOperand for Ordering {
    fn from_value(value: i64) -> Self {
        assert!((0..=3).contains(&value));
        Self::from_bits(value & 2 != 0, value & 1 != 0)
    }
}

#[test]
fn all_opcodes_match_gnu_assembler() {
    let mut covered = HashSet::new();
    let mut count = 0;
    for line in include_str!("testdata/gnu_encodings.tsv")
        .lines()
        .filter(|line| !line.starts_with('#'))
    {
        let columns: Vec<_> = line.split('\t').collect();
        let opcode: Opcode = columns[0].parse().unwrap();
        let xlen = if columns[1] == "32" {
            Xlen::X32
        } else {
            Xlen::X64
        };
        let expected = u32::from_str_radix(columns[2], 16).unwrap();
        let operands: Vec<i64> = columns[3]
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.parse().unwrap())
            .collect();
        let instruction = opcode.test_instruction(&operands);
        let encoded = instruction
            .encode_for(xlen)
            .unwrap_or_else(|e| panic!("{line}: {e}"));
        assert_eq!(encoded.bits(), expected, "{}: {instruction:?}", columns[4]);
        assert_eq!(encoded.byte_len(), opcode.byte_len());
        covered.insert(opcode);
        count += 1;
    }
    assert_eq!(covered.len(), Opcode::ALL.len());
    assert_eq!(count, 1568);
}

#[test]
fn random_generation_encodes_every_supported_opcode_for_both_xlens() {
    let mut rng = StdRng::seed_from_u64(0xc0decafe);
    for xlen in [Xlen::X32, Xlen::X64] {
        for &opcode in Opcode::ALL {
            if !opcode.supports_xlen(xlen) {
                assert!(opcode.random(&mut rng, xlen).is_err());
                continue;
            }
            for _ in 0..128 {
                let instruction = opcode.random(&mut rng, xlen).unwrap();
                assert_eq!(instruction.opcode(), opcode);
                let encoded = instruction
                    .encode_for(xlen)
                    .unwrap_or_else(|e| panic!("{instruction:?}: {e}"));
                assert_eq!(encoded.byte_len(), opcode.byte_len());
                assert_eq!(encoded.bits() & 3 == 3, opcode.byte_len() == 4);
            }
        }
    }
}

#[test]
fn typed_operands_reject_invalid_architectural_values() {
    assert!(XReg::new(32).is_err());
    assert!(FReg::new(255).is_err());
    assert!(Csr::new(4096).is_err());
    for rm in [5, 6, 8, 255] {
        assert!(RoundingMode::try_from(rm).is_err());
    }
    assert_eq!(XReg::A0.index(), 10);
    assert_eq!(FReg::FT11.index(), 31);
    assert_eq!(XReg::FP, XReg::S0);
    assert_eq!(XReg::SP.abi_name(), "sp");
    assert_eq!(Csr::MSTATUS.index(), 0x300);
    assert_eq!(Csr::MCONTEXT.index(), 0x7a8);
}

#[test]
fn invalid_immediates_register_subsets_and_xlen_are_rejected() {
    use Instruction::*;
    let invalid = [
        Addi {
            rd: XReg::A0,
            rs1: XReg::A1,
            imm: 2048,
        },
        Addi {
            rd: XReg::A0,
            rs1: XReg::A1,
            imm: -2049,
        },
        Lui {
            rd: XReg::A0,
            imm: 1048576,
        },
        Jal {
            rd: XReg::RA,
            imm: 3,
        },
        Jal {
            rd: XReg::RA,
            imm: 1048576,
        },
        Beq {
            rs1: XReg::A0,
            rs2: XReg::A1,
            imm: -4098,
        },
        Beq {
            rs1: XReg::A0,
            rs2: XReg::A1,
            imm: 1,
        },
        Slli {
            rd: XReg::A0,
            rs1: XReg::A1,
            shamt: 64,
        },
        Slliw {
            rd: XReg::A0,
            rs1: XReg::A1,
            shamt: 32,
        },
        Csrrwi {
            rd: XReg::A0,
            csr: Csr::MSTATUS,
            uimm: 32,
        },
        Fence { imm: 4096 },
        CLw {
            rd: XReg::T0,
            rs1: XReg::A0,
            imm: 0,
        },
        CSw {
            rs1: XReg::A0,
            rs2: XReg::A6,
            imm: 0,
        },
        CSw {
            rs1: XReg::A0,
            rs2: XReg::A1,
            imm: 2,
        },
        CAddi {
            rd: XReg::RA,
            imm: 32,
        },
        CAddi4spn {
            rd: XReg::A0,
            imm: 0,
        },
        CAddi16sp { imm: 0 },
        CAddi16sp { imm: 8 },
        CAddiw {
            rd: XReg::ZERO,
            imm: 1,
        },
        CLui {
            rd: XReg::SP,
            imm: 1,
        },
        CLui {
            rd: XReg::RA,
            imm: 0,
        },
        CJ { imm: -2049 },
        CBeqz {
            rs1: XReg::A0,
            imm: 256,
        },
        CJr { rs1: XReg::ZERO },
        CJalr { rs1: XReg::ZERO },
        CMv {
            rd: XReg::RA,
            rs2: XReg::ZERO,
        },
        CLwsp {
            rd: XReg::ZERO,
            imm: 0,
        },
        CSdsp {
            rs2: XReg::A0,
            imm: 4,
        },
        CLd {
            rd: XReg::A0,
            rs1: XReg::A1,
            imm: 256,
        },
        CSlli {
            rd: XReg::RA,
            shamt: 0,
        },
    ];
    for instruction in invalid {
        assert!(instruction.encode().is_err(), "{instruction:?}");
    }
    let shift = Slli {
        rd: XReg::A0,
        rs1: XReg::A1,
        shamt: 32,
    };
    assert!(shift.encode_for(Xlen::X32).is_err());
    assert!(shift.encode_for(Xlen::X64).is_ok());
    let shift = CSrli {
        rs1: XReg::A0,
        shamt: 63,
    };
    assert!(shift.encode_for(Xlen::X32).is_err());
    assert!(shift.encode_for(Xlen::X64).is_ok());
    assert!(CJal { imm: 2 }.encode_for(Xlen::X64).is_err());
    assert!(
        Ld {
            rd: XReg::A0,
            rs1: XReg::A1,
            imm: 0
        }
        .encode_for(Xlen::X32)
        .is_err()
    );
    assert!(
        CAddiw {
            rd: XReg::RA,
            imm: 0
        }
        .encode_for(Xlen::X32)
        .is_err()
    );
}

#[test]
fn mixed_width_blocks_support_values_boxes_and_operand_edits() {
    let mut block = vec![
        Instruction::nop(),
        Instruction::cnop(),
        Instruction::CEbreak,
    ];
    block[0] = Instruction::Addi {
        rd: XReg::RA,
        rs1: XReg::ZERO,
        imm: 1,
    };
    if let Instruction::Addi { imm, .. } = &mut block[0] {
        *imm = -1;
    }
    let mut bytes = Vec::new();
    for instruction in &block {
        instruction.append_bytes(&mut bytes).unwrap();
    }
    assert_eq!(bytes, [0x93, 0x00, 0xf0, 0xff, 0x01, 0x00, 0x02, 0x90]);
    let boxed: Vec<Box<Instruction>> = block.into_iter().map(Box::new).collect();
    assert_eq!(boxed[2].encode().unwrap().to_string(), ".2byte 0x9002");
    assert_eq!(boxed[0].encode().unwrap().to_string(), ".4byte 0xfff00093");
    let original = bytes.clone();
    let invalid = Instruction::CJ { imm: 1 };
    assert!(invalid.append_bytes(&mut bytes).is_err());
    assert_eq!(bytes, original);
    assert_eq!(
        Instruction::standard(0xdeadbeef)
            .encode()
            .unwrap()
            .to_le_bytes(),
        [0xef, 0xbe, 0xad, 0xde]
    );
    assert_eq!(
        Instruction::compressed(0xffff)
            .encode()
            .unwrap()
            .to_le_bytes(),
        [0xff, 0xff]
    );
    assert!(std::mem::size_of::<Instruction>() <= 24);
}

#[test]
fn opcode_metadata_and_atomic_bits() {
    let mut names = HashSet::new();
    for &opcode in Opcode::ALL {
        assert!(names.insert(opcode.mnemonic()));
        assert_eq!(opcode.mnemonic().parse::<Opcode>().unwrap(), opcode);
        assert!(opcode.class().opcodes().any(|op| op == opcode));
    }
    assert!("invalid".parse::<Opcode>().is_err());
    assert_eq!(Opcode::SinvalVma.extension(), Extension::Svinval);
    for aq in [false, true] {
        for rl in [false, true] {
            let ordering = Ordering::from_bits(aq, rl);
            let i = Instruction::LrW {
                rd: XReg::T0,
                rs1: XReg::T1,
                ordering,
            };
            let bits = i.encode().unwrap().bits();
            assert_eq!(bits >> 26 & 1 != 0, aq);
            assert_eq!(bits >> 25 & 1 != 0, rl);
            assert_eq!(bits >> 20 & 31, 0);
        }
    }
    let dependency = Instruction::Lw {
        rd: XReg::A0,
        rs1: XReg::A1,
        imm: 0,
    }
    .syntactic_dependency()
    .unwrap();
    assert_eq!(dependency.sources, &[rvwmo::RegType::Address]);
    assert_eq!(dependency.destinations, &[rvwmo::RegType::Standard]);
    assert!(Instruction::cnop().syntactic_dependency().is_none());
}

#[test]
fn load_immediates_reconstruct_boundary_values_on_rv32_and_rv64() {
    for value in [
        0,
        1,
        -1,
        0x7ff,
        0x800,
        0xfff,
        0x12345678,
        i32::MAX,
        i32::MIN,
    ] {
        for xlen in [Xlen::X32, Xlen::X64] {
            let sequence = asmutil::load_imm32(XReg::A0, value, xlen);
            let upper = sequence[0].encode_for(xlen).unwrap().bits() & 0xfffff000;
            let low = (sequence[1].encode_for(xlen).unwrap().bits() as i32) >> 20;
            let result = match sequence[1] {
                Instruction::Addiw { .. } => (upper as i32 as i64 + low as i64) as i32 as i64,
                Instruction::Addi { .. } => (upper as i32).wrapping_add(low) as i64,
                _ => panic!("unexpected load-immediate sequence"),
            };
            assert_eq!(result, value as i64);
        }
    }
    assert_eq!(asmutil::twos_complement(u64::MAX, Xlen::X64), -1);
    assert_eq!(asmutil::to_unsigned(-1, Xlen::X32), 0xffffffff);
}
