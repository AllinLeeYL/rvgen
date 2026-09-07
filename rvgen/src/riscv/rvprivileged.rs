// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

//! Port of `riscv/rvprivileged.py`; function names and argument order are retained.

use super::rvprotoinstrs::*;

pub const RV32I_OPCODE_PRIVILEGED: i32 = 115;

pub fn rvprivileged_sret() -> u32 {
    instruc_rtype(RV32I_OPCODE_PRIVILEGED, 0, 0, 0, 2, 8)
}

pub fn rvprivileged_mret() -> u32 {
    instruc_rtype(RV32I_OPCODE_PRIVILEGED, 0, 0, 0, 2, 24)
}

pub fn rvprivileged_wfi() -> u32 {
    instruc_rtype(RV32I_OPCODE_PRIVILEGED, 0, 0, 0, 5, 8)
}

pub fn rvprivileged_sfence_vma(rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32I_OPCODE_PRIVILEGED, 0, 0, rs1, rs2, 9)
}

pub fn rvprivileged_sinval_vma(rs1: i32, rs2: i32) -> u32 {
    instruc_rtype(RV32I_OPCODE_PRIVILEGED, 0, 0, rs1, rs2, 11)
}

pub fn rvprivileged_sfence_w_inval() -> u32 {
    instruc_rtype(RV32I_OPCODE_PRIVILEGED, 0, 0, 0, 0, 12)
}

pub fn rvprivileged_sfence_inval_ir() -> u32 {
    instruc_rtype(RV32I_OPCODE_PRIVILEGED, 0, 0, 0, 1, 12)
}
