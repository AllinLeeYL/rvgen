use rvgen::riscv::{asmutil::*, registers::*, rvwmo::*};

#[test]
fn immediate_split_reconstructs_constants() {
    for value in [0, 1, 0x7ff, 0x800, 0xfff, 0x1000, 0x1234_5678, 0x7fff_f7ff] {
        let (lui, addi) = li_into_reg(value, true);
        assert!((-2048..2048).contains(&addi));
        assert_eq!((lui as i64 * 4096 + addi as i64) as u64, value);
    }
}

#[test]
fn signed_conversions_cover_boundaries() {
    for value in [0, 1, 0x7fff_ffff, 0x8000_0000, 0xffff_ffff] {
        assert_eq!(to_unsigned(twos_complement(value, false), false), value);
    }
    for value in [0, 1, i64::MAX as u64, 1 << 63, u64::MAX] {
        assert_eq!(to_unsigned(twos_complement(value, true), true), value);
    }
}

#[test]
fn register_names_and_dependency_metadata_are_available() {
    assert_eq!(INTREG_ABINAMES.len(), 32);
    assert_eq!(IntReg::s10.abi_name(), "s10");
    assert_eq!(IntReg::s11.abi_name(), "s11");
    assert_eq!(IntReg::t3.abi_name(), "t3");
    assert_eq!(FloatReg::ft11.abi_name(), "ft11");
    let store = syntactic_dependency("sw").unwrap();
    assert_eq!(
        store.sources,
        Some([RegType::Address, RegType::Data].as_slice())
    );
    assert!(store.destinations.is_none());
    assert!(syntactic_dependency("fdiv.d")
        .unwrap()
        .accumulating_csrs
        .unwrap()
        .contains(&AccCsr::DZ));
    assert!(syntactic_dependency("unknown").is_none());
    let names: std::collections::HashSet<_> = SYNTACTIC_DEP_PROPAGUATION
        .iter()
        .map(|(name, _)| name)
        .collect();
    assert_eq!(names.len(), SYNTACTIC_DEP_PROPAGUATION.len());
    assert_eq!(
        syntactic_dependency("csrrs").unwrap().dependency_flags,
        (true, true)
    );
    assert_eq!(RegType::Data as u8, 1);
}

#[test]
#[should_panic]
fn rejects_invalid_register() {
    rvgen::riscv::rv32i_add(32, 0, 0);
}

#[test]
#[should_panic]
fn rejects_negative_register() {
    rvgen::riscv::rv32i_add(-1, 0, 0);
}
