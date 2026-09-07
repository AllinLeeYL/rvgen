// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

//! Port of `riscv/zifencei.py`; function names and argument order are retained.

use super::rvprotoinstrs::*;

pub const ZIFENCEI_OPCODE_FENCEI: i32 = 15;

pub fn zifencei_fencei(imm: i32) -> u32 {
    instruc_itype(ZIFENCEI_OPCODE_FENCEI, 0, 1, 0, imm)
}
