// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

/// Split an unsigned constant into LUI and sign-extended ADDI immediates.
/// The optional bound check matches the Python helper's 31-bit restriction.
pub fn li_into_reg(value: u64, do_check_bounds: bool) -> (u32, i32) {
    if do_check_bounds {
        assert!(
            value < 0x8000_0000,
            "sign extension beyond 31 bits is not implemented"
        );
    }
    let carry = (value >> 11) & 1;
    let addi = (value & 0xfff) as i32 - (carry as i32 * 0x1000);
    let lui = ((value >> 12) + carry) & 0xfffff;
    (lui as u32, addi)
}

pub fn twos_complement(value: u64, is_rv64: bool) -> i64 {
    if is_rv64 {
        value as i64
    } else {
        assert!(value <= u32::MAX as u64);
        value as u32 as i32 as i64
    }
}

pub fn to_unsigned(value: i64, is_rv64: bool) -> u64 {
    if is_rv64 {
        value as u64
    } else {
        assert!((i32::MIN as i64..=u32::MAX as i64).contains(&value));
        value as u32 as u64
    }
}
