// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only
//! ABI register aliases and architectural constants from the references.
use super::fields::{Csr, FReg, XReg};
use rand::{Rng, RngExt};
impl XReg {
    pub const X0: Self = Self::constant(0);
    pub const X1: Self = Self::constant(1);
    pub const X2: Self = Self::constant(2);
    pub const X3: Self = Self::constant(3);
    pub const X4: Self = Self::constant(4);
    pub const X5: Self = Self::constant(5);
    pub const X6: Self = Self::constant(6);
    pub const X7: Self = Self::constant(7);
    pub const X8: Self = Self::constant(8);
    pub const X9: Self = Self::constant(9);
    pub const X10: Self = Self::constant(10);
    pub const X11: Self = Self::constant(11);
    pub const X12: Self = Self::constant(12);
    pub const X13: Self = Self::constant(13);
    pub const X14: Self = Self::constant(14);
    pub const X15: Self = Self::constant(15);
    pub const X16: Self = Self::constant(16);
    pub const X17: Self = Self::constant(17);
    pub const X18: Self = Self::constant(18);
    pub const X19: Self = Self::constant(19);
    pub const X20: Self = Self::constant(20);
    pub const X21: Self = Self::constant(21);
    pub const X22: Self = Self::constant(22);
    pub const X23: Self = Self::constant(23);
    pub const X24: Self = Self::constant(24);
    pub const X25: Self = Self::constant(25);
    pub const X26: Self = Self::constant(26);
    pub const X27: Self = Self::constant(27);
    pub const X28: Self = Self::constant(28);
    pub const X29: Self = Self::constant(29);
    pub const X30: Self = Self::constant(30);
    pub const X31: Self = Self::constant(31);
    pub const ZERO: Self = Self::X0;
    pub const RA: Self = Self::X1;
    pub const SP: Self = Self::X2;
    pub const GP: Self = Self::X3;
    pub const TP: Self = Self::X4;
    pub const T0: Self = Self::X5;
    pub const T1: Self = Self::X6;
    pub const T2: Self = Self::X7;
    pub const S0: Self = Self::X8;
    pub const S1: Self = Self::X9;
    pub const A0: Self = Self::X10;
    pub const A1: Self = Self::X11;
    pub const A2: Self = Self::X12;
    pub const A3: Self = Self::X13;
    pub const A4: Self = Self::X14;
    pub const A5: Self = Self::X15;
    pub const A6: Self = Self::X16;
    pub const A7: Self = Self::X17;
    pub const S2: Self = Self::X18;
    pub const S3: Self = Self::X19;
    pub const S4: Self = Self::X20;
    pub const S5: Self = Self::X21;
    pub const S6: Self = Self::X22;
    pub const S7: Self = Self::X23;
    pub const S8: Self = Self::X24;
    pub const S9: Self = Self::X25;
    pub const S10: Self = Self::X26;
    pub const S11: Self = Self::X27;
    pub const T3: Self = Self::X28;
    pub const T4: Self = Self::X29;
    pub const T5: Self = Self::X30;
    pub const T6: Self = Self::X31;
    pub const FP: Self = Self::S0;
    pub const ABI_NAMES: [&'static str; 32] = [
        "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1", "a0", "a1", "a2", "a3", "a4",
        "a5", "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "t3", "t4",
        "t5", "t6",
    ];
    pub const fn abi_name(self) -> &'static str {
        Self::ABI_NAMES[self.index() as usize]
    }
}
impl FReg {
    pub const F0: Self = Self::constant(0);
    pub const F1: Self = Self::constant(1);
    pub const F2: Self = Self::constant(2);
    pub const F3: Self = Self::constant(3);
    pub const F4: Self = Self::constant(4);
    pub const F5: Self = Self::constant(5);
    pub const F6: Self = Self::constant(6);
    pub const F7: Self = Self::constant(7);
    pub const F8: Self = Self::constant(8);
    pub const F9: Self = Self::constant(9);
    pub const F10: Self = Self::constant(10);
    pub const F11: Self = Self::constant(11);
    pub const F12: Self = Self::constant(12);
    pub const F13: Self = Self::constant(13);
    pub const F14: Self = Self::constant(14);
    pub const F15: Self = Self::constant(15);
    pub const F16: Self = Self::constant(16);
    pub const F17: Self = Self::constant(17);
    pub const F18: Self = Self::constant(18);
    pub const F19: Self = Self::constant(19);
    pub const F20: Self = Self::constant(20);
    pub const F21: Self = Self::constant(21);
    pub const F22: Self = Self::constant(22);
    pub const F23: Self = Self::constant(23);
    pub const F24: Self = Self::constant(24);
    pub const F25: Self = Self::constant(25);
    pub const F26: Self = Self::constant(26);
    pub const F27: Self = Self::constant(27);
    pub const F28: Self = Self::constant(28);
    pub const F29: Self = Self::constant(29);
    pub const F30: Self = Self::constant(30);
    pub const F31: Self = Self::constant(31);
    pub const FT0: Self = Self::F0;
    pub const FT1: Self = Self::F1;
    pub const FT2: Self = Self::F2;
    pub const FT3: Self = Self::F3;
    pub const FT4: Self = Self::F4;
    pub const FT5: Self = Self::F5;
    pub const FT6: Self = Self::F6;
    pub const FT7: Self = Self::F7;
    pub const FS0: Self = Self::F8;
    pub const FS1: Self = Self::F9;
    pub const FA0: Self = Self::F10;
    pub const FA1: Self = Self::F11;
    pub const FA2: Self = Self::F12;
    pub const FA3: Self = Self::F13;
    pub const FA4: Self = Self::F14;
    pub const FA5: Self = Self::F15;
    pub const FA6: Self = Self::F16;
    pub const FA7: Self = Self::F17;
    pub const FS2: Self = Self::F18;
    pub const FS3: Self = Self::F19;
    pub const FS4: Self = Self::F20;
    pub const FS5: Self = Self::F21;
    pub const FS6: Self = Self::F22;
    pub const FS7: Self = Self::F23;
    pub const FS8: Self = Self::F24;
    pub const FS9: Self = Self::F25;
    pub const FS10: Self = Self::F26;
    pub const FS11: Self = Self::F27;
    pub const FT8: Self = Self::F28;
    pub const FT9: Self = Self::F29;
    pub const FT10: Self = Self::F30;
    pub const FT11: Self = Self::F31;
    pub const ABI_NAMES: [&'static str; 32] = [
        "ft0", "ft1", "ft2", "ft3", "ft4", "ft5", "ft6", "ft7", "fs0", "fs1", "fa0", "fa1", "fa2",
        "fa3", "fa4", "fa5", "fa6", "fa7", "fs2", "fs3", "fs4", "fs5", "fs6", "fs7", "fs8", "fs9",
        "fs10", "fs11", "ft8", "ft9", "ft10", "ft11",
    ];
    pub const fn abi_name(self) -> &'static str {
        Self::ABI_NAMES[self.index() as usize]
    }
}
impl Csr {
    pub const USTATUS: Self = Self::constant(0x000);
    pub const UIE: Self = Self::constant(0x004);
    pub const UTVEC: Self = Self::constant(0x005);
    pub const USCRATCH: Self = Self::constant(0x040);
    pub const UEPC: Self = Self::constant(0x041);
    pub const UCAUSE: Self = Self::constant(0x042);
    pub const UTVAL: Self = Self::constant(0x043);
    pub const UIP: Self = Self::constant(0x044);
    pub const FFLAGS: Self = Self::constant(0x001);
    pub const FRM: Self = Self::constant(0x002);
    pub const FCSR: Self = Self::constant(0x003);
    pub const CYCLE: Self = Self::constant(0xc00);
    pub const TIME: Self = Self::constant(0xc01);
    pub const INSTRET: Self = Self::constant(0xc02);
    pub const HPMCOUNTER3: Self = Self::constant(0xc03);
    pub const HPMCOUNTER4: Self = Self::constant(0xc04);
    pub const HPMCOUNTER5: Self = Self::constant(0xc05);
    pub const HPMCOUNTER6: Self = Self::constant(0xc06);
    pub const HPMCOUNTER7: Self = Self::constant(0xc07);
    pub const HPMCOUNTER8: Self = Self::constant(0xc08);
    pub const HPMCOUNTER9: Self = Self::constant(0xc09);
    pub const HPMCOUNTER10: Self = Self::constant(0xc0a);
    pub const HPMCOUNTER11: Self = Self::constant(0xc0b);
    pub const HPMCOUNTER12: Self = Self::constant(0xc0c);
    pub const HPMCOUNTER13: Self = Self::constant(0xc0d);
    pub const HPMCOUNTER14: Self = Self::constant(0xc0e);
    pub const HPMCOUNTER15: Self = Self::constant(0xc0f);
    pub const HPMCOUNTER16: Self = Self::constant(0xc10);
    pub const HPMCOUNTER17: Self = Self::constant(0xc11);
    pub const HPMCOUNTER18: Self = Self::constant(0xc12);
    pub const HPMCOUNTER19: Self = Self::constant(0xc13);
    pub const HPMCOUNTER20: Self = Self::constant(0xc14);
    pub const HPMCOUNTER21: Self = Self::constant(0xc15);
    pub const HPMCOUNTER22: Self = Self::constant(0xc16);
    pub const HPMCOUNTER23: Self = Self::constant(0xc17);
    pub const HPMCOUNTER24: Self = Self::constant(0xc18);
    pub const HPMCOUNTER25: Self = Self::constant(0xc19);
    pub const HPMCOUNTER26: Self = Self::constant(0xc1a);
    pub const HPMCOUNTER27: Self = Self::constant(0xc1b);
    pub const HPMCOUNTER28: Self = Self::constant(0xc1c);
    pub const HPMCOUNTER29: Self = Self::constant(0xc1d);
    pub const HPMCOUNTER30: Self = Self::constant(0xc1e);
    pub const HPMCOUNTER31: Self = Self::constant(0xc1f);
    pub const CYCLEH: Self = Self::constant(0xc80);
    pub const TIMEH: Self = Self::constant(0xc81);
    pub const INSTRETH: Self = Self::constant(0xc82);
    pub const HPMCOUNTERH3: Self = Self::constant(0xc83);
    pub const HPMCOUNTERH4: Self = Self::constant(0xc84);
    pub const HPMCOUNTERH5: Self = Self::constant(0xc85);
    pub const HPMCOUNTERH6: Self = Self::constant(0xc86);
    pub const HPMCOUNTERH7: Self = Self::constant(0xc87);
    pub const HPMCOUNTERH8: Self = Self::constant(0xc88);
    pub const HPMCOUNTERH9: Self = Self::constant(0xc89);
    pub const HPMCOUNTERH10: Self = Self::constant(0xc8a);
    pub const HPMCOUNTERH11: Self = Self::constant(0xc8b);
    pub const HPMCOUNTERH12: Self = Self::constant(0xc8c);
    pub const HPMCOUNTERH13: Self = Self::constant(0xc8d);
    pub const HPMCOUNTERH14: Self = Self::constant(0xc8e);
    pub const HPMCOUNTERH15: Self = Self::constant(0xc8f);
    pub const HPMCOUNTERH16: Self = Self::constant(0xc90);
    pub const HPMCOUNTERH17: Self = Self::constant(0xc91);
    pub const HPMCOUNTERH18: Self = Self::constant(0xc92);
    pub const HPMCOUNTERH19: Self = Self::constant(0xc93);
    pub const HPMCOUNTERH20: Self = Self::constant(0xc94);
    pub const HPMCOUNTERH21: Self = Self::constant(0xc95);
    pub const HPMCOUNTERH22: Self = Self::constant(0xc96);
    pub const HPMCOUNTERH23: Self = Self::constant(0xc97);
    pub const HPMCOUNTERH24: Self = Self::constant(0xc98);
    pub const HPMCOUNTERH25: Self = Self::constant(0xc99);
    pub const HPMCOUNTERH26: Self = Self::constant(0xc9a);
    pub const HPMCOUNTERH27: Self = Self::constant(0xc9b);
    pub const HPMCOUNTERH28: Self = Self::constant(0xc9c);
    pub const HPMCOUNTERH29: Self = Self::constant(0xc9d);
    pub const HPMCOUNTERH30: Self = Self::constant(0xc9e);
    pub const HPMCOUNTERH31: Self = Self::constant(0xc9f);
    pub const SSTATUS: Self = Self::constant(0x100);
    pub const SEDELEG: Self = Self::constant(0x102);
    pub const SIDELEG: Self = Self::constant(0x103);
    pub const SIE: Self = Self::constant(0x104);
    pub const STVEC: Self = Self::constant(0x105);
    pub const SCOUNTEREN: Self = Self::constant(0x106);
    pub const SENCFG: Self = Self::constant(0x10a);
    pub const SSCRATCH: Self = Self::constant(0x140);
    pub const SEPC: Self = Self::constant(0x141);
    pub const SCAUSE: Self = Self::constant(0x142);
    pub const STVAL: Self = Self::constant(0x143);
    pub const SIP: Self = Self::constant(0x144);
    pub const SATP: Self = Self::constant(0x180);
    pub const SCONTEXT: Self = Self::constant(0x5a8);
    pub const MVENDORID: Self = Self::constant(0xf11);
    pub const MARCHID: Self = Self::constant(0xf12);
    pub const MIMPID: Self = Self::constant(0xf13);
    pub const MHARTID: Self = Self::constant(0xf14);
    pub const MCONFIGPTR: Self = Self::constant(0xf15);
    pub const MSTATUS: Self = Self::constant(0x300);
    pub const MISA: Self = Self::constant(0x301);
    pub const MEDELEG: Self = Self::constant(0x302);
    pub const MIDELEG: Self = Self::constant(0x303);
    pub const MIE: Self = Self::constant(0x304);
    pub const MTVEC: Self = Self::constant(0x305);
    pub const MCOUNTEREN: Self = Self::constant(0x306);
    pub const MSTATUSH: Self = Self::constant(0x310);
    pub const MSCRATCH: Self = Self::constant(0x340);
    pub const MEPC: Self = Self::constant(0x341);
    pub const MCAUSE: Self = Self::constant(0x342);
    pub const MTVAL: Self = Self::constant(0x343);
    pub const MIP: Self = Self::constant(0x344);
    pub const MTINST: Self = Self::constant(0x34a);
    pub const MTVAL2: Self = Self::constant(0x34b);
    pub const MENVCFG: Self = Self::constant(0x30a);
    pub const MENVCFGH: Self = Self::constant(0x31a);
    pub const MSECCFG: Self = Self::constant(0x747);
    pub const MSECCFGH: Self = Self::constant(0x757);
    pub const PMPCFG0: Self = Self::constant(0x3a0);
    pub const PMPCFG1: Self = Self::constant(0x3a1);
    pub const PMPCFG2: Self = Self::constant(0x3a2);
    pub const PMPCFG3: Self = Self::constant(0x3a3);
    pub const PMPCFG4: Self = Self::constant(0x3a4);
    pub const PMPCFG5: Self = Self::constant(0x3a5);
    pub const PMPCFG6: Self = Self::constant(0x3a6);
    pub const PMPCFG7: Self = Self::constant(0x3a7);
    pub const PMPCFG8: Self = Self::constant(0x3a8);
    pub const PMPCFG9: Self = Self::constant(0x3a9);
    pub const PMPCFGA: Self = Self::constant(0x3aa);
    pub const PMPCFGB: Self = Self::constant(0x3ab);
    pub const PMPCFGC: Self = Self::constant(0x3ac);
    pub const PMPCFGD: Self = Self::constant(0x3ad);
    pub const PMPCFGE: Self = Self::constant(0x3ae);
    pub const PMPCFGF: Self = Self::constant(0x3af);
    pub const PMPADDR0: Self = Self::constant(0x3b0);
    pub const PMPADDR1: Self = Self::constant(0x3b1);
    pub const PMPADDR2: Self = Self::constant(0x3b2);
    pub const PMPADDR3: Self = Self::constant(0x3b3);
    pub const PMPADDR4: Self = Self::constant(0x3b4);
    pub const PMPADDR5: Self = Self::constant(0x3b5);
    pub const PMPADDR6: Self = Self::constant(0x3b6);
    pub const PMPADDR7: Self = Self::constant(0x3b7);
    pub const PMPADDR8: Self = Self::constant(0x3b8);
    pub const PMPADDR9: Self = Self::constant(0x3b9);
    pub const PMPADDRA: Self = Self::constant(0x3ba);
    pub const PMPADDRB: Self = Self::constant(0x3bb);
    pub const PMPADDRC: Self = Self::constant(0x3bc);
    pub const PMPADDRD: Self = Self::constant(0x3bd);
    pub const PMPADDRE: Self = Self::constant(0x3be);
    pub const PMPADDRF: Self = Self::constant(0x3bf);
    pub const PMPADDR10: Self = Self::constant(0x3c0);
    pub const PMPADDR11: Self = Self::constant(0x3c1);
    pub const PMPADDR12: Self = Self::constant(0x3c2);
    pub const PMPADDR13: Self = Self::constant(0x3c3);
    pub const PMPADDR14: Self = Self::constant(0x3c4);
    pub const PMPADDR15: Self = Self::constant(0x3c5);
    pub const PMPADDR16: Self = Self::constant(0x3c6);
    pub const PMPADDR17: Self = Self::constant(0x3c7);
    pub const PMPADDR18: Self = Self::constant(0x3c8);
    pub const PMPADDR19: Self = Self::constant(0x3c9);
    pub const PMPADDR1A: Self = Self::constant(0x3ca);
    pub const PMPADDR1B: Self = Self::constant(0x3cb);
    pub const PMPADDR1C: Self = Self::constant(0x3cc);
    pub const PMPADDR1D: Self = Self::constant(0x3cd);
    pub const PMPADDR1E: Self = Self::constant(0x3ce);
    pub const PMPADDR1F: Self = Self::constant(0x3cf);
    pub const PMPADDR20: Self = Self::constant(0x3d0);
    pub const PMPADDR21: Self = Self::constant(0x3d1);
    pub const PMPADDR22: Self = Self::constant(0x3d2);
    pub const PMPADDR23: Self = Self::constant(0x3d3);
    pub const PMPADDR24: Self = Self::constant(0x3d4);
    pub const PMPADDR25: Self = Self::constant(0x3d5);
    pub const PMPADDR26: Self = Self::constant(0x3d6);
    pub const PMPADDR27: Self = Self::constant(0x3d7);
    pub const PMPADDR28: Self = Self::constant(0x3d8);
    pub const PMPADDR29: Self = Self::constant(0x3d9);
    pub const PMPADDR2A: Self = Self::constant(0x3da);
    pub const PMPADDR2B: Self = Self::constant(0x3db);
    pub const PMPADDR2C: Self = Self::constant(0x3dc);
    pub const PMPADDR2D: Self = Self::constant(0x3dd);
    pub const PMPADDR2E: Self = Self::constant(0x3de);
    pub const PMPADDR2F: Self = Self::constant(0x3df);
    pub const PMPADDR30: Self = Self::constant(0x3e0);
    pub const PMPADDR31: Self = Self::constant(0x3e1);
    pub const PMPADDR32: Self = Self::constant(0x3e2);
    pub const PMPADDR33: Self = Self::constant(0x3e3);
    pub const PMPADDR34: Self = Self::constant(0x3e4);
    pub const PMPADDR35: Self = Self::constant(0x3e5);
    pub const PMPADDR36: Self = Self::constant(0x3e6);
    pub const PMPADDR37: Self = Self::constant(0x3e7);
    pub const PMPADDR38: Self = Self::constant(0x3e8);
    pub const PMPADDR39: Self = Self::constant(0x3e9);
    pub const PMPADDR3A: Self = Self::constant(0x3ea);
    pub const PMPADDR3B: Self = Self::constant(0x3eb);
    pub const PMPADDR3C: Self = Self::constant(0x3ec);
    pub const PMPADDR3D: Self = Self::constant(0x3ed);
    pub const PMPADDR3E: Self = Self::constant(0x3ee);
    pub const PMPADDR3F: Self = Self::constant(0x3ef);
    pub const MCYCLE: Self = Self::constant(0xb00);
    pub const MINSTRET: Self = Self::constant(0xb02);
    pub const MHPMCOUNTER3: Self = Self::constant(0xb03);
    pub const MHPMCOUNTER4: Self = Self::constant(0xb04);
    pub const MHPMCOUNTER5: Self = Self::constant(0xb05);
    pub const MHPMCOUNTER6: Self = Self::constant(0xb06);
    pub const MHPMCOUNTER7: Self = Self::constant(0xb07);
    pub const MHPMCOUNTER8: Self = Self::constant(0xb08);
    pub const MHPMCOUNTER9: Self = Self::constant(0xb09);
    pub const MHPMCOUNTER10: Self = Self::constant(0xb0a);
    pub const MHPMCOUNTER11: Self = Self::constant(0xb0b);
    pub const MHPMCOUNTER12: Self = Self::constant(0xb0c);
    pub const MHPMCOUNTER13: Self = Self::constant(0xb0d);
    pub const MHPMCOUNTER14: Self = Self::constant(0xb0e);
    pub const MHPMCOUNTER15: Self = Self::constant(0xb0f);
    pub const MHPMCOUNTER16: Self = Self::constant(0xb10);
    pub const MHPMCOUNTER17: Self = Self::constant(0xb11);
    pub const MHPMCOUNTER18: Self = Self::constant(0xb12);
    pub const MHPMCOUNTER19: Self = Self::constant(0xb13);
    pub const MHPMCOUNTER20: Self = Self::constant(0xb14);
    pub const MHPMCOUNTER21: Self = Self::constant(0xb15);
    pub const MHPMCOUNTER22: Self = Self::constant(0xb16);
    pub const MHPMCOUNTER23: Self = Self::constant(0xb17);
    pub const MHPMCOUNTER24: Self = Self::constant(0xb18);
    pub const MHPMCOUNTER25: Self = Self::constant(0xb19);
    pub const MHPMCOUNTER26: Self = Self::constant(0xb1a);
    pub const MHPMCOUNTER27: Self = Self::constant(0xb1b);
    pub const MHPMCOUNTER28: Self = Self::constant(0xb1c);
    pub const MHPMCOUNTER29: Self = Self::constant(0xb1d);
    pub const MHPMCOUNTER30: Self = Self::constant(0xb1e);
    pub const MHPMCOUNTER31: Self = Self::constant(0xb1f);
    pub const MCYCLEH: Self = Self::constant(0xb80);
    pub const MINSTRETH: Self = Self::constant(0xb82);
    pub const MHPMCOUNTERH3: Self = Self::constant(0xb83);
    pub const MHPMCOUNTERH4: Self = Self::constant(0xb84);
    pub const MHPMCOUNTERH5: Self = Self::constant(0xb85);
    pub const MHPMCOUNTERH6: Self = Self::constant(0xb86);
    pub const MHPMCOUNTERH7: Self = Self::constant(0xb87);
    pub const MHPMCOUNTERH8: Self = Self::constant(0xb88);
    pub const MHPMCOUNTERH9: Self = Self::constant(0xb89);
    pub const MHPMCOUNTERH10: Self = Self::constant(0xb8a);
    pub const MHPMCOUNTERH11: Self = Self::constant(0xb8b);
    pub const MHPMCOUNTERH12: Self = Self::constant(0xb8c);
    pub const MHPMCOUNTERH13: Self = Self::constant(0xb8d);
    pub const MHPMCOUNTERH14: Self = Self::constant(0xb8e);
    pub const MHPMCOUNTERH15: Self = Self::constant(0xb8f);
    pub const MHPMCOUNTERH16: Self = Self::constant(0xb90);
    pub const MHPMCOUNTERH17: Self = Self::constant(0xb91);
    pub const MHPMCOUNTERH18: Self = Self::constant(0xb92);
    pub const MHPMCOUNTERH19: Self = Self::constant(0xb93);
    pub const MHPMCOUNTERH20: Self = Self::constant(0xb94);
    pub const MHPMCOUNTERH21: Self = Self::constant(0xb95);
    pub const MHPMCOUNTERH22: Self = Self::constant(0xb96);
    pub const MHPMCOUNTERH23: Self = Self::constant(0xb97);
    pub const MHPMCOUNTERH24: Self = Self::constant(0xb98);
    pub const MHPMCOUNTERH25: Self = Self::constant(0xb99);
    pub const MHPMCOUNTERH26: Self = Self::constant(0xb9a);
    pub const MHPMCOUNTERH27: Self = Self::constant(0xb9b);
    pub const MHPMCOUNTERH28: Self = Self::constant(0xb9c);
    pub const MHPMCOUNTERH29: Self = Self::constant(0xb9d);
    pub const MHPMCOUNTERH30: Self = Self::constant(0xb9e);
    pub const MHPMCOUNTERH31: Self = Self::constant(0xb9f);
    pub const MCOUNTINHIBIT: Self = Self::constant(0x320);
    pub const MHPMEVENT3: Self = Self::constant(0x323);
    pub const MHPMEVENT4: Self = Self::constant(0x324);
    pub const MHPMEVENT5: Self = Self::constant(0x325);
    pub const MHPMEVENT6: Self = Self::constant(0x326);
    pub const MHPMEVENT7: Self = Self::constant(0x327);
    pub const MHPMEVENT8: Self = Self::constant(0x328);
    pub const MHPMEVENT9: Self = Self::constant(0x329);
    pub const MHPMEVENT10: Self = Self::constant(0x32a);
    pub const MHPMEVENT11: Self = Self::constant(0x32b);
    pub const MHPMEVENT12: Self = Self::constant(0x32c);
    pub const MHPMEVENT13: Self = Self::constant(0x32d);
    pub const MHPMEVENT14: Self = Self::constant(0x32e);
    pub const MHPMEVENT15: Self = Self::constant(0x32f);
    pub const MHPMEVENT16: Self = Self::constant(0x330);
    pub const MHPMEVENT17: Self = Self::constant(0x331);
    pub const MHPMEVENT18: Self = Self::constant(0x332);
    pub const MHPMEVENT19: Self = Self::constant(0x333);
    pub const MHPMEVENT20: Self = Self::constant(0x334);
    pub const MHPMEVENT21: Self = Self::constant(0x335);
    pub const MHPMEVENT22: Self = Self::constant(0x336);
    pub const MHPMEVENT23: Self = Self::constant(0x337);
    pub const MHPMEVENT24: Self = Self::constant(0x338);
    pub const MHPMEVENT25: Self = Self::constant(0x339);
    pub const MHPMEVENT26: Self = Self::constant(0x33a);
    pub const MHPMEVENT27: Self = Self::constant(0x33b);
    pub const MHPMEVENT28: Self = Self::constant(0x33c);
    pub const MHPMEVENT29: Self = Self::constant(0x33d);
    pub const MHPMEVENT30: Self = Self::constant(0x33e);
    pub const MHPMEVENT31: Self = Self::constant(0x33f);
    pub const TSELECT: Self = Self::constant(0x7a0);
    pub const TDATA1: Self = Self::constant(0x7a1);
    pub const TDATA2: Self = Self::constant(0x7a2);
    pub const TDATA3: Self = Self::constant(0x7a3);
    // RISC-V Debug Specification: Machine Context (mcontext), CSR 0x7a8.
    pub const MCONTEXT: Self = Self::constant(0x7a8);
}
pub const ILEN: u32 = 4;
pub const WORD_SIZE: u32 = 4;
pub const DWORD_SIZE: u32 = 8;
/// log2 of the base instruction alignment in bytes, without C.
pub const INSTRUCTION_ALIGNMENT_LOG2: u32 = 2;
pub const MAX_DEVICES: u32 = 1023;
pub const MAX_HARTS: u32 = 15872;
pub const PRIORITY_BASE: u32 = 0;
pub const PENDING_BASE: u32 = 4096;
pub const ENABLE_BASE: u32 = 8192;
pub const HART_BASE: u32 = 2097152;
pub const MSIP_WIDTH: u32 = 4;
pub const MTIMER_OFFSET: u32 = 16384;
pub const SPIKE_CLINT_BASE: u32 = 33554432;
pub const INT_REGISTERS: &[XReg] = &[
    XReg::T0,
    XReg::T1,
    XReg::T2,
    XReg::T3,
    XReg::T4,
    XReg::T5,
    XReg::T6,
    XReg::A0,
    XReg::A1,
    XReg::A2,
    XReg::A3,
    XReg::A4,
    XReg::A5,
    XReg::A6,
    XReg::A7,
];
pub const FLOAT_REGISTERS: &[FReg] = &[
    FReg::FT0,
    FReg::FT1,
    FReg::FT2,
    FReg::FT3,
    FReg::FT4,
    FReg::FT5,
    FReg::FT6,
    FReg::FT7,
    FReg::FT8,
    FReg::FT9,
    FReg::FT10,
    FReg::FT11,
    FReg::FA0,
    FReg::FA1,
    FReg::FA2,
    FReg::FA3,
    FReg::FA4,
    FReg::FA5,
    FReg::FA6,
    FReg::FA7,
];
pub fn random_int_register(rng: &mut (impl Rng + ?Sized)) -> XReg {
    INT_REGISTERS[rng.random_range(0..INT_REGISTERS.len())]
}
pub fn random_float_register(rng: &mut (impl Rng + ?Sized)) -> FReg {
    FLOAT_REGISTERS[rng.random_range(0..FLOAT_REGISTERS.len())]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PrivilegeLevel {
    User = 0,
    Supervisor = 1,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ExceptionCause {
    InstructionAddressMisaligned = 0,
    InstructionAccessFault = 1,
    IllegalInstruction = 2,
    Breakpoint = 3,
    LoadAddressMisaligned = 4,
    LoadAccessFault = 5,
    StoreAmoAddressMisaligned = 6,
    StoreAmoAccessFault = 7,
    EnvironmentCallFromUser = 8,
    EnvironmentCallFromSupervisor = 9,
    EnvironmentCallFromMachine = 11,
    InstructionPageFault = 12,
    LoadPageFault = 13,
    StoreAmoPageFault = 15,
}

pub const INTERESTING_CSRS_INACCESSIBLE_FROM_SUPERVISOR: &[Csr] = &[
    Csr::MVENDORID,
    Csr::MARCHID,
    Csr::MIMPID,
    Csr::MHARTID,
    Csr::MCONFIGPTR,
    Csr::MSTATUS,
    Csr::MISA,
    Csr::MEDELEG,
    Csr::MIDELEG,
    Csr::MIE,
    Csr::MTVEC,
    Csr::MCOUNTEREN,
    Csr::MSTATUSH,
    Csr::MSCRATCH,
    Csr::MEPC,
    Csr::MCAUSE,
    Csr::MTVAL,
    Csr::MIP,
    Csr::MTINST,
    Csr::MTVAL2,
    Csr::MENVCFG,
    Csr::MENVCFGH,
    Csr::MSECCFG,
    Csr::MSECCFGH,
    Csr::PMPCFG0,
    Csr::PMPADDR0,
    Csr::MCYCLE,
    Csr::MINSTRET,
    Csr::MCYCLEH,
    Csr::MINSTRETH,
    Csr::MHPMCOUNTERH3,
    Csr::MCOUNTINHIBIT,
    Csr::MHPMEVENT3,
    Csr::TSELECT,
    Csr::TDATA1,
    Csr::TDATA2,
    Csr::TDATA3,
    Csr::MCONTEXT,
];

pub const INTERESTING_CSRS_INACCESSIBLE_FROM_USER: &[Csr] = &[
    Csr::MVENDORID,
    Csr::MARCHID,
    Csr::MIMPID,
    Csr::MHARTID,
    Csr::MCONFIGPTR,
    Csr::MSTATUS,
    Csr::MISA,
    Csr::MEDELEG,
    Csr::MIDELEG,
    Csr::MIE,
    Csr::MTVEC,
    Csr::MCOUNTEREN,
    Csr::MSTATUSH,
    Csr::MSCRATCH,
    Csr::MEPC,
    Csr::MCAUSE,
    Csr::MTVAL,
    Csr::MIP,
    Csr::MTINST,
    Csr::MTVAL2,
    Csr::MENVCFG,
    Csr::MENVCFGH,
    Csr::MSECCFG,
    Csr::MSECCFGH,
    Csr::PMPCFG0,
    Csr::PMPADDR0,
    Csr::MCYCLE,
    Csr::MINSTRET,
    Csr::MCYCLEH,
    Csr::MINSTRETH,
    Csr::MHPMCOUNTERH3,
    Csr::MCOUNTINHIBIT,
    Csr::MHPMEVENT3,
    Csr::TSELECT,
    Csr::TDATA1,
    Csr::TDATA2,
    Csr::TDATA3,
    Csr::MCONTEXT,
    Csr::SSTATUS,
    Csr::SEDELEG,
    Csr::SIDELEG,
    Csr::SIE,
    Csr::STVEC,
    Csr::SCOUNTEREN,
    Csr::SENCFG,
    Csr::SSCRATCH,
    Csr::SEPC,
    Csr::SCAUSE,
    Csr::STVAL,
    Csr::SIP,
    Csr::SATP,
    Csr::SCONTEXT,
];
