// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

#![allow(non_camel_case_types)]

pub const MAX_DEVICES: usize = 1023;
pub const MAX_HARTS: usize = 15872;
pub const PRIORITY_BASE: usize = 0;
pub const PENDING_BASE: usize = 4096;
pub const ENABLE_BASE: usize = 8192;
pub const HART_BASE: usize = 2097152;
pub const MSIP_WIDTH: usize = 4;
pub const MTIMER_OFFSET: usize = 16384;
pub const SPIKE_CLINT_BASE: usize = 33554432;
