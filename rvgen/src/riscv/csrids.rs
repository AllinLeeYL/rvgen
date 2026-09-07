// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

#![allow(non_camel_case_types)]

/// CSR identifiers. A namespace struct preserves aliases with equal values.
pub struct CSR;
impl CSR {
    pub const USTATUS: i32 = 0x000;
    pub const UIE: i32 = 0x004;
    pub const UTVEC: i32 = 0x005;
    pub const USCRATCH: i32 = 0x040;
    pub const UEPC: i32 = 0x041;
    pub const UCAUSE: i32 = 0x042;
    pub const UTVAL: i32 = 0x043;
    pub const UIP: i32 = 0x044;
    pub const FFLAGS: i32 = 0x001;
    pub const FRM: i32 = 0x002;
    pub const FCSR: i32 = 0x003;
    pub const CYCLE: i32 = 0xc00;
    pub const TIME: i32 = 0xc01;
    pub const INSTRET: i32 = 0xc02;
    pub const HPMCOUNTER3: i32 = 0xc03;
    pub const HPMCOUNTER4: i32 = 0xc04;
    pub const HPMCOUNTER5: i32 = 0xc05;
    pub const HPMCOUNTER6: i32 = 0xc06;
    pub const HPMCOUNTER7: i32 = 0xc07;
    pub const HPMCOUNTER8: i32 = 0xc08;
    pub const HPMCOUNTER9: i32 = 0xc09;
    pub const HPMCOUNTER10: i32 = 0xc0a;
    pub const HPMCOUNTER11: i32 = 0xc0b;
    pub const HPMCOUNTER12: i32 = 0xc0c;
    pub const HPMCOUNTER13: i32 = 0xc0d;
    pub const HPMCOUNTER14: i32 = 0xc0e;
    pub const HPMCOUNTER15: i32 = 0xc0f;
    pub const HPMCOUNTER16: i32 = 0xc10;
    pub const HPMCOUNTER17: i32 = 0xc11;
    pub const HPMCOUNTER18: i32 = 0xc12;
    pub const HPMCOUNTER19: i32 = 0xc13;
    pub const HPMCOUNTER20: i32 = 0xc14;
    pub const HPMCOUNTER21: i32 = 0xc15;
    pub const HPMCOUNTER22: i32 = 0xc16;
    pub const HPMCOUNTER23: i32 = 0xc17;
    pub const HPMCOUNTER24: i32 = 0xc18;
    pub const HPMCOUNTER25: i32 = 0xc19;
    pub const HPMCOUNTER26: i32 = 0xc1a;
    pub const HPMCOUNTER27: i32 = 0xc1b;
    pub const HPMCOUNTER28: i32 = 0xc1c;
    pub const HPMCOUNTER29: i32 = 0xc1d;
    pub const HPMCOUNTER30: i32 = 0xc1e;
    pub const HPMCOUNTER31: i32 = 0xc1f;
    pub const CYCLEH: i32 = 0xc80;
    pub const TIMEH: i32 = 0xc81;
    pub const INSTRETH: i32 = 0xc82;
    pub const HPMCOUNTERH3: i32 = 0xc83;
    pub const HPMCOUNTERH4: i32 = 0xc84;
    pub const HPMCOUNTERH5: i32 = 0xc85;
    pub const HPMCOUNTERH6: i32 = 0xc86;
    pub const HPMCOUNTERH7: i32 = 0xc87;
    pub const HPMCOUNTERH8: i32 = 0xc88;
    pub const HPMCOUNTERH9: i32 = 0xc89;
    pub const HPMCOUNTERH10: i32 = 0xc8a;
    pub const HPMCOUNTERH11: i32 = 0xc8b;
    pub const HPMCOUNTERH12: i32 = 0xc8c;
    pub const HPMCOUNTERH13: i32 = 0xc8d;
    pub const HPMCOUNTERH14: i32 = 0xc8e;
    pub const HPMCOUNTERH15: i32 = 0xc8f;
    pub const HPMCOUNTERH16: i32 = 0xc90;
    pub const HPMCOUNTERH17: i32 = 0xc91;
    pub const HPMCOUNTERH18: i32 = 0xc92;
    pub const HPMCOUNTERH19: i32 = 0xc93;
    pub const HPMCOUNTERH20: i32 = 0xc94;
    pub const HPMCOUNTERH21: i32 = 0xc95;
    pub const HPMCOUNTERH22: i32 = 0xc96;
    pub const HPMCOUNTERH23: i32 = 0xc97;
    pub const HPMCOUNTERH24: i32 = 0xc98;
    pub const HPMCOUNTERH25: i32 = 0xc99;
    pub const HPMCOUNTERH26: i32 = 0xc9a;
    pub const HPMCOUNTERH27: i32 = 0xc9b;
    pub const HPMCOUNTERH28: i32 = 0xc9c;
    pub const HPMCOUNTERH29: i32 = 0xc9d;
    pub const HPMCOUNTERH30: i32 = 0xc9e;
    pub const HPMCOUNTERH31: i32 = 0xc9f;
    pub const SSTATUS: i32 = 0x100;
    pub const SEDELEG: i32 = 0x102;
    pub const SIDELEG: i32 = 0x103;
    pub const SIE: i32 = 0x104;
    pub const STVEC: i32 = 0x105;
    pub const SCOUNTEREN: i32 = 0x106;
    pub const SENCFG: i32 = 0x10a;
    pub const SSCRATCH: i32 = 0x140;
    pub const SEPC: i32 = 0x141;
    pub const SCAUSE: i32 = 0x142;
    pub const STVAL: i32 = 0x143;
    pub const SIP: i32 = 0x144;
    pub const SATP: i32 = 0x180;
    pub const SCONTEXT: i32 = 0x5a8;
    pub const MVENDORID: i32 = 0xf11;
    pub const MARCHID: i32 = 0xf12;
    pub const MIMPID: i32 = 0xf13;
    pub const MHARTID: i32 = 0xf14;
    pub const MCONFIGPTR: i32 = 0xf15;
    pub const MSTATUS: i32 = 0x300;
    pub const MISA: i32 = 0x301;
    pub const MEDELEG: i32 = 0x302;
    pub const MIDELEG: i32 = 0x303;
    pub const MIE: i32 = 0x304;
    pub const MTVEC: i32 = 0x305;
    pub const MCOUNTEREN: i32 = 0x306;
    pub const MSTATUSH: i32 = 0x310;
    pub const MSCRATCH: i32 = 0x340;
    pub const MEPC: i32 = 0x341;
    pub const MCAUSE: i32 = 0x342;
    pub const MTVAL: i32 = 0x343;
    pub const MIP: i32 = 0x344;
    pub const MTINST: i32 = 0x34a;
    pub const MTVAL2: i32 = 0x34b;
    pub const MENVCFG: i32 = 0x30a;
    pub const MENVCFGH: i32 = 0x31a;
    pub const MSECCFG: i32 = 0x747;
    pub const MSECCFGH: i32 = 0x757;
    pub const PMPCFG0: i32 = 0x3a0;
    pub const PMPCFG1: i32 = 0x3a1;
    pub const PMPCFG2: i32 = 0x3a2;
    pub const PMPCFG3: i32 = 0x3a3;
    pub const PMPCFG4: i32 = 0x3a4;
    pub const PMPCFG5: i32 = 0x3a5;
    pub const PMPCFG6: i32 = 0x3a6;
    pub const PMPCFG7: i32 = 0x3a7;
    pub const PMPCFG8: i32 = 0x3a8;
    pub const PMPCFG9: i32 = 0x3a9;
    pub const PMPCFGA: i32 = 0x3aa;
    pub const PMPCFGB: i32 = 0x3ab;
    pub const PMPCFGC: i32 = 0x3ac;
    pub const PMPCFGD: i32 = 0x3ad;
    pub const PMPCFGE: i32 = 0x3ae;
    pub const PMPCFGF: i32 = 0x3af;
    pub const PMPADDR0: i32 = 0x3b0;
    pub const PMPADDR1: i32 = 0x3b1;
    pub const PMPADDR2: i32 = 0x3b2;
    pub const PMPADDR3: i32 = 0x3b3;
    pub const PMPADDR4: i32 = 0x3b4;
    pub const PMPADDR5: i32 = 0x3b5;
    pub const PMPADDR6: i32 = 0x3b6;
    pub const PMPADDR7: i32 = 0x3b7;
    pub const PMPADDR8: i32 = 0x3b8;
    pub const PMPADDR9: i32 = 0x3b9;
    pub const PMPADDRA: i32 = 0x3ba;
    pub const PMPADDRB: i32 = 0x3bb;
    pub const PMPADDRC: i32 = 0x3bc;
    pub const PMPADDRD: i32 = 0x3bd;
    pub const PMPADDRE: i32 = 0x3be;
    pub const PMPADDRF: i32 = 0x3bf;
    pub const PMPADDR10: i32 = 0x3c0;
    pub const PMPADDR11: i32 = 0x3c1;
    pub const PMPADDR12: i32 = 0x3c2;
    pub const PMPADDR13: i32 = 0x3c3;
    pub const PMPADDR14: i32 = 0x3c4;
    pub const PMPADDR15: i32 = 0x3c5;
    pub const PMPADDR16: i32 = 0x3c6;
    pub const PMPADDR17: i32 = 0x3c7;
    pub const PMPADDR18: i32 = 0x3c8;
    pub const PMPADDR19: i32 = 0x3c9;
    pub const PMPADDR1A: i32 = 0x3ca;
    pub const PMPADDR1B: i32 = 0x3cb;
    pub const PMPADDR1C: i32 = 0x3cc;
    pub const PMPADDR1D: i32 = 0x3cd;
    pub const PMPADDR1E: i32 = 0x3ce;
    pub const PMPADDR1F: i32 = 0x3cf;
    pub const PMPADDR20: i32 = 0x3d0;
    pub const PMPADDR21: i32 = 0x3d1;
    pub const PMPADDR22: i32 = 0x3d2;
    pub const PMPADDR23: i32 = 0x3d3;
    pub const PMPADDR24: i32 = 0x3d4;
    pub const PMPADDR25: i32 = 0x3d5;
    pub const PMPADDR26: i32 = 0x3d6;
    pub const PMPADDR27: i32 = 0x3d7;
    pub const PMPADDR28: i32 = 0x3d8;
    pub const PMPADDR29: i32 = 0x3d9;
    pub const PMPADDR2A: i32 = 0x3da;
    pub const PMPADDR2B: i32 = 0x3db;
    pub const PMPADDR2C: i32 = 0x3dc;
    pub const PMPADDR2D: i32 = 0x3dd;
    pub const PMPADDR2E: i32 = 0x3de;
    pub const PMPADDR2F: i32 = 0x3df;
    pub const PMPADDR30: i32 = 0x3e0;
    pub const PMPADDR31: i32 = 0x3e1;
    pub const PMPADDR32: i32 = 0x3e2;
    pub const PMPADDR33: i32 = 0x3e3;
    pub const PMPADDR34: i32 = 0x3e4;
    pub const PMPADDR35: i32 = 0x3e5;
    pub const PMPADDR36: i32 = 0x3e6;
    pub const PMPADDR37: i32 = 0x3e7;
    pub const PMPADDR38: i32 = 0x3e8;
    pub const PMPADDR39: i32 = 0x3e9;
    pub const PMPADDR3A: i32 = 0x3ea;
    pub const PMPADDR3B: i32 = 0x3eb;
    pub const PMPADDR3C: i32 = 0x3ec;
    pub const PMPADDR3D: i32 = 0x3ed;
    pub const PMPADDR3E: i32 = 0x3ee;
    pub const PMPADDR3F: i32 = 0x3ef;
    pub const MCYCLE: i32 = 0xb00;
    pub const MINSTRET: i32 = 0xb02;
    pub const MHPMCOUNTER3: i32 = 0xb03;
    pub const MHPMCOUNTER4: i32 = 0xb04;
    pub const MHPMCOUNTER5: i32 = 0xb05;
    pub const MHPMCOUNTER6: i32 = 0xb06;
    pub const MHPMCOUNTER7: i32 = 0xb07;
    pub const MHPMCOUNTER8: i32 = 0xb08;
    pub const MHPMCOUNTER9: i32 = 0xb09;
    pub const MHPMCOUNTER10: i32 = 0xb0a;
    pub const MHPMCOUNTER11: i32 = 0xb0b;
    pub const MHPMCOUNTER12: i32 = 0xb0c;
    pub const MHPMCOUNTER13: i32 = 0xb0d;
    pub const MHPMCOUNTER14: i32 = 0xb0e;
    pub const MHPMCOUNTER15: i32 = 0xb0f;
    pub const MHPMCOUNTER16: i32 = 0xb10;
    pub const MHPMCOUNTER17: i32 = 0xb11;
    pub const MHPMCOUNTER18: i32 = 0xb12;
    pub const MHPMCOUNTER19: i32 = 0xb13;
    pub const MHPMCOUNTER20: i32 = 0xb14;
    pub const MHPMCOUNTER21: i32 = 0xb15;
    pub const MHPMCOUNTER22: i32 = 0xb16;
    pub const MHPMCOUNTER23: i32 = 0xb17;
    pub const MHPMCOUNTER24: i32 = 0xb18;
    pub const MHPMCOUNTER25: i32 = 0xb19;
    pub const MHPMCOUNTER26: i32 = 0xb1a;
    pub const MHPMCOUNTER27: i32 = 0xb1b;
    pub const MHPMCOUNTER28: i32 = 0xb1c;
    pub const MHPMCOUNTER29: i32 = 0xb1d;
    pub const MHPMCOUNTER30: i32 = 0xb1e;
    pub const MHPMCOUNTER31: i32 = 0xb1f;
    pub const MCYCLEH: i32 = 0xb80;
    pub const MINSTRETH: i32 = 0xb82;
    pub const MHPMCOUNTERH3: i32 = 0xb83;
    pub const MHPMCOUNTERH4: i32 = 0xb84;
    pub const MHPMCOUNTERH5: i32 = 0xb85;
    pub const MHPMCOUNTERH6: i32 = 0xb86;
    pub const MHPMCOUNTERH7: i32 = 0xb87;
    pub const MHPMCOUNTERH8: i32 = 0xb88;
    pub const MHPMCOUNTERH9: i32 = 0xb89;
    pub const MHPMCOUNTERH10: i32 = 0xb8a;
    pub const MHPMCOUNTERH11: i32 = 0xb8b;
    pub const MHPMCOUNTERH12: i32 = 0xb8c;
    pub const MHPMCOUNTERH13: i32 = 0xb8d;
    pub const MHPMCOUNTERH14: i32 = 0xb8e;
    pub const MHPMCOUNTERH15: i32 = 0xb8f;
    pub const MHPMCOUNTERH16: i32 = 0xb90;
    pub const MHPMCOUNTERH17: i32 = 0xb91;
    pub const MHPMCOUNTERH18: i32 = 0xb92;
    pub const MHPMCOUNTERH19: i32 = 0xb93;
    pub const MHPMCOUNTERH20: i32 = 0xb94;
    pub const MHPMCOUNTERH21: i32 = 0xb95;
    pub const MHPMCOUNTERH22: i32 = 0xb96;
    pub const MHPMCOUNTERH23: i32 = 0xb97;
    pub const MHPMCOUNTERH24: i32 = 0xb98;
    pub const MHPMCOUNTERH25: i32 = 0xb99;
    pub const MHPMCOUNTERH26: i32 = 0xb9a;
    pub const MHPMCOUNTERH27: i32 = 0xb9b;
    pub const MHPMCOUNTERH28: i32 = 0xb9c;
    pub const MHPMCOUNTERH29: i32 = 0xb9d;
    pub const MHPMCOUNTERH30: i32 = 0xb9e;
    pub const MHPMCOUNTERH31: i32 = 0xb9f;
    pub const MCOUNTINHIBIT: i32 = 0x320;
    pub const MHPMEVENT3: i32 = 0x323;
    pub const MHPMEVENT4: i32 = 0x324;
    pub const MHPMEVENT5: i32 = 0x325;
    pub const MHPMEVENT6: i32 = 0x326;
    pub const MHPMEVENT7: i32 = 0x327;
    pub const MHPMEVENT8: i32 = 0x328;
    pub const MHPMEVENT9: i32 = 0x329;
    pub const MHPMEVENT10: i32 = 0x32a;
    pub const MHPMEVENT11: i32 = 0x32b;
    pub const MHPMEVENT12: i32 = 0x32c;
    pub const MHPMEVENT13: i32 = 0x32d;
    pub const MHPMEVENT14: i32 = 0x32e;
    pub const MHPMEVENT15: i32 = 0x32f;
    pub const MHPMEVENT16: i32 = 0x330;
    pub const MHPMEVENT17: i32 = 0x331;
    pub const MHPMEVENT18: i32 = 0x332;
    pub const MHPMEVENT19: i32 = 0x333;
    pub const MHPMEVENT20: i32 = 0x334;
    pub const MHPMEVENT21: i32 = 0x335;
    pub const MHPMEVENT22: i32 = 0x336;
    pub const MHPMEVENT23: i32 = 0x337;
    pub const MHPMEVENT24: i32 = 0x338;
    pub const MHPMEVENT25: i32 = 0x339;
    pub const MHPMEVENT26: i32 = 0x33a;
    pub const MHPMEVENT27: i32 = 0x33b;
    pub const MHPMEVENT28: i32 = 0x33c;
    pub const MHPMEVENT29: i32 = 0x33d;
    pub const MHPMEVENT30: i32 = 0x33e;
    pub const MHPMEVENT31: i32 = 0x33f;
    pub const TSELECT: i32 = 0x7a0;
    pub const TDATA1: i32 = 0x7a1;
    pub const TDATA2: i32 = 0x7a2;
    pub const TDATA3: i32 = 0x7a3;
    pub const MCONTEXT: i32 = 0x7a3;
}

pub const INTERESTING_CSRS_INACCESSIBLE_FROM_SUPERVISOR: &[i32] = &[
    CSR::MVENDORID,
    CSR::MARCHID,
    CSR::MIMPID,
    CSR::MHARTID,
    CSR::MCONFIGPTR,
    CSR::MSTATUS,
    CSR::MISA,
    CSR::MEDELEG,
    CSR::MIDELEG,
    CSR::MIE,
    CSR::MTVEC,
    CSR::MCOUNTEREN,
    CSR::MSTATUSH,
    CSR::MSCRATCH,
    CSR::MEPC,
    CSR::MCAUSE,
    CSR::MTVAL,
    CSR::MIP,
    CSR::MTINST,
    CSR::MTVAL2,
    CSR::MENVCFG,
    CSR::MENVCFGH,
    CSR::MSECCFG,
    CSR::MSECCFGH,
    CSR::PMPCFG0,
    CSR::PMPADDR0,
    CSR::MCYCLE,
    CSR::MINSTRET,
    CSR::MCYCLEH,
    CSR::MINSTRETH,
    CSR::MHPMCOUNTERH3,
    CSR::MCOUNTINHIBIT,
    CSR::MHPMEVENT3,
    CSR::TSELECT,
    CSR::TDATA1,
    CSR::TDATA2,
    CSR::TDATA3,
    CSR::MCONTEXT,
];
pub const INTERESTING_CSRS_INACCESSIBLE_FROM_USER: &[i32] = &[
    CSR::MVENDORID,
    CSR::MARCHID,
    CSR::MIMPID,
    CSR::MHARTID,
    CSR::MCONFIGPTR,
    CSR::MSTATUS,
    CSR::MISA,
    CSR::MEDELEG,
    CSR::MIDELEG,
    CSR::MIE,
    CSR::MTVEC,
    CSR::MCOUNTEREN,
    CSR::MSTATUSH,
    CSR::MSCRATCH,
    CSR::MEPC,
    CSR::MCAUSE,
    CSR::MTVAL,
    CSR::MIP,
    CSR::MTINST,
    CSR::MTVAL2,
    CSR::MENVCFG,
    CSR::MENVCFGH,
    CSR::MSECCFG,
    CSR::MSECCFGH,
    CSR::PMPCFG0,
    CSR::PMPADDR0,
    CSR::MCYCLE,
    CSR::MINSTRET,
    CSR::MCYCLEH,
    CSR::MINSTRETH,
    CSR::MHPMCOUNTERH3,
    CSR::MCOUNTINHIBIT,
    CSR::MHPMEVENT3,
    CSR::TSELECT,
    CSR::TDATA1,
    CSR::TDATA2,
    CSR::TDATA3,
    CSR::MCONTEXT,
    CSR::SSTATUS,
    CSR::SEDELEG,
    CSR::SIDELEG,
    CSR::SIE,
    CSR::STVEC,
    CSR::SCOUNTEREN,
    CSR::SENCFG,
    CSR::SSCRATCH,
    CSR::SEPC,
    CSR::SCAUSE,
    CSR::STVAL,
    CSR::SIP,
    CSR::SATP,
    CSR::SCONTEXT,
];
