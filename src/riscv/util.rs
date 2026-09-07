// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

#![allow(non_camel_case_types)]

pub const ILEN: usize = 4;
pub const WORD_SIZE: usize = 4;
pub const DWORD_SIZE: usize = 8;
pub const BYTES_ALLIGN_4: usize = 2;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ExceptionCause {
    ID_INSTR_ADDR_MISALIGNED = 0,
    ID_INSTR_ACCESS_FAULT = 1,
    ID_ILLEGAL_INSTRUCTION = 2,
    ID_BREAKPOINT = 3,
    ID_LOAD_ADDR_MISALIGNED = 4,
    ID_LOAD_ACCESS_FAULT = 5,
    ID_STORE_AMO_ADDR_MISALIGNED = 6,
    ID_STORE_AMO_ACCESS_FAULT = 7,
    ID_ENVIRONMENT_CALL_FROM_U_MODE = 8,
    ID_ENVIRONMENT_CALL_FROM_S_MODE = 9,
    ID_ENVIRONMENT_CALL_FROM_M_MODE = 11,
    ID_INSTRUCTION_PAGE_FAULT = 12,
    ID_LOAD_PAGE_FAULT = 13,
    ID_STORE_AMO_PAGE_FAULT = 15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PrivLvl {
    User = 0,
    Supervisor = 1,
    Hypervisor = 2,
    Machine = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FpuState {
    Off = 0,
    Init = 1,
    Clean = 2,
    Dirty = 3,
}

pub const ROUNDING_MODES: &[i32] = &[0, 1, 2, 3, 4];
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum FenceOrdering {
    RW_RW = 51,
    TSO = 2099,
    RW_W = 49,
    R_RW = 35,
    R_R = 34,
    W_W = 17,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Ordering {
    NoOrd = 0,
    Rel = 1,
    Acq = 2,
    Seq = 3,
}

impl Ordering {
    pub fn bits(self) -> (bool, bool) {
        match self {
            Self::NoOrd => (false, false),
            Self::Rel => (false, true),
            Self::Acq => (true, false),
            Self::Seq => (true, true),
        }
    }
}
