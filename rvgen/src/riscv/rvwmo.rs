// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

//! Syntactic dependency metadata from the original RVWMO table.
//! This table describes operands; it is not a memory-model checker.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RegType {
    Data = 1,
    Address = 2,
    Standard = 3,
    Csr = 4,
    Frm = 5,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AccCsr {
    NV = 1,
    OF = 2,
    UF = 3,
    NX = 4,
    DZ = 5,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryInstructionKind {
    IntLoadInstr,
    FloatLoadInstr,
    IntStoreInstr,
    FloatStoreInstr,
    AmoInstr,
    AmoStore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntacticDependency {
    pub sources: Option<&'static [RegType]>,
    pub destinations: Option<&'static [RegType]>,
    pub accumulating_csrs: Option<&'static [AccCsr]>,
    /// The final two flags from the Python table, in their original order.
    pub dependency_flags: (bool, bool),
}

pub const LOAD_INSTR: &[MemoryInstructionKind] = &[
    MemoryInstructionKind::IntLoadInstr,
    MemoryInstructionKind::FloatLoadInstr,
];

pub const STORE_INSTR: &[MemoryInstructionKind] = &[
    MemoryInstructionKind::IntStoreInstr,
    MemoryInstructionKind::FloatStoreInstr,
];

pub const AMO_INSTR: &[MemoryInstructionKind] = &[
    MemoryInstructionKind::AmoInstr,
    MemoryInstructionKind::AmoStore,
];

pub const MEMOP: &[MemoryInstructionKind] = &[
    MemoryInstructionKind::IntLoadInstr,
    MemoryInstructionKind::FloatLoadInstr,
    MemoryInstructionKind::IntStoreInstr,
    MemoryInstructionKind::FloatStoreInstr,
    MemoryInstructionKind::AmoInstr,
    MemoryInstructionKind::AmoStore,
];

pub const FRM_FLOAT: &[&str] = &[
    "fmadd.s",
    "fmsub.s",
    "fnmsub.s",
    "fnmadd.s",
    "fmadd.d",
    "fmsub.d",
    "fnmsub.d",
    "fnmadd.d",
    "fadd.s",
    "fsub.s",
    "fmul.s",
    "fdiv.s",
    "fadd.d",
    "fsub.d",
    "fmul.d",
    "fdiv.d",
    "fsqrt.s",
    "fsqrt.d",
    "fcvt.s.d",
    "fcvt.w.s",
    "fcvt.wu.s",
    "fcvt.l.s",
    "fcvt.lu.s",
    "fcvt.w.d",
    "fcvt.wu.d",
    "fcvt.l.d",
    "fcvt.lu.d",
    "fcvt.s.w",
    "fcvt.s.wu",
    "fcvt.s.l",
    "fcvt.s.lu",
    "fcvt.d.l",
    "fcvt.d.lu",
];

pub const SYNTACTIC_DEP_PROPAGUATION: &[(&str, SyntacticDependency)] = &[
    (
        "lui",
        SyntacticDependency {
            sources: None,
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "auipc",
        SyntacticDependency {
            sources: None,
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "jal",
        SyntacticDependency {
            sources: None,
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "jalr",
        SyntacticDependency {
            sources: Some(&[RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "beq",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: None,
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "bne",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: None,
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "blt",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: None,
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "bge",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: None,
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "bltu",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: None,
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "bgeu",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: None,
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "lb",
        SyntacticDependency {
            sources: Some(&[RegType::Address]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "lh",
        SyntacticDependency {
            sources: Some(&[RegType::Address]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "lw",
        SyntacticDependency {
            sources: Some(&[RegType::Address]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "lbu",
        SyntacticDependency {
            sources: Some(&[RegType::Address]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "lhu",
        SyntacticDependency {
            sources: Some(&[RegType::Address]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "sb",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: None,
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "sh",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: None,
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "sw",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: None,
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "addi",
        SyntacticDependency {
            sources: Some(&[RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "slti",
        SyntacticDependency {
            sources: Some(&[RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "sltiu",
        SyntacticDependency {
            sources: Some(&[RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "xori",
        SyntacticDependency {
            sources: Some(&[RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "ori",
        SyntacticDependency {
            sources: Some(&[RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "andi",
        SyntacticDependency {
            sources: Some(&[RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "slli",
        SyntacticDependency {
            sources: Some(&[RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "srli",
        SyntacticDependency {
            sources: Some(&[RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "srai",
        SyntacticDependency {
            sources: Some(&[RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "add",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "sub",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "sll",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "slt",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "sltu",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "xor",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "srl",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "sra",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "or",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "and",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "fence",
        SyntacticDependency {
            sources: None,
            destinations: None,
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "fence.i",
        SyntacticDependency {
            sources: None,
            destinations: None,
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "ecall",
        SyntacticDependency {
            sources: None,
            destinations: None,
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "ebreak",
        SyntacticDependency {
            sources: None,
            destinations: None,
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "csrrw",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Csr]),
            destinations: Some(&[RegType::Standard, RegType::Csr]),
            accumulating_csrs: None,
            dependency_flags: (true, true),
        },
    ),
    (
        "csrrs",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Csr]),
            destinations: Some(&[RegType::Standard, RegType::Csr]),
            accumulating_csrs: None,
            dependency_flags: (true, true),
        },
    ),
    (
        "csrrc",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Csr]),
            destinations: Some(&[RegType::Standard, RegType::Csr]),
            accumulating_csrs: None,
            dependency_flags: (true, true),
        },
    ),
    (
        "csrrwi",
        SyntacticDependency {
            sources: Some(&[RegType::Csr]),
            destinations: Some(&[RegType::Standard, RegType::Csr]),
            accumulating_csrs: None,
            dependency_flags: (true, true),
        },
    ),
    (
        "csrrsi",
        SyntacticDependency {
            sources: Some(&[RegType::Csr]),
            destinations: Some(&[RegType::Standard, RegType::Csr]),
            accumulating_csrs: None,
            dependency_flags: (true, true),
        },
    ),
    (
        "csrrci",
        SyntacticDependency {
            sources: Some(&[RegType::Csr]),
            destinations: Some(&[RegType::Standard, RegType::Csr]),
            accumulating_csrs: None,
            dependency_flags: (true, true),
        },
    ),
    (
        "lwu",
        SyntacticDependency {
            sources: Some(&[RegType::Address]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "ld",
        SyntacticDependency {
            sources: Some(&[RegType::Address]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "sd",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: None,
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "addiw",
        SyntacticDependency {
            sources: Some(&[RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "slliw",
        SyntacticDependency {
            sources: Some(&[RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "srliw",
        SyntacticDependency {
            sources: Some(&[RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "sraiw",
        SyntacticDependency {
            sources: Some(&[RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "addw",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "subw",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "sllw",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "srlw",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "sraw",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "mul",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "mulh",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "mulhsu",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "mulhu",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "div",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "divu",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "rem",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "remu",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "mulw",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "divw",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "divuw",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "remw",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "remuw",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "lr.w",
        SyntacticDependency {
            sources: Some(&[RegType::Address]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "sc.w",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (true, false),
        },
    ),
    (
        "amoswap.w",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "amoadd.w",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "amoxor.w",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "amoand.w",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "amoor.w",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "amomin.w",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "amomax.w",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "amominu.w",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "amomaxu.w",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "lr.d",
        SyntacticDependency {
            sources: Some(&[RegType::Address]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "sc.d",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (true, false),
        },
    ),
    (
        "amoswap.d",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "amoadd.d",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "amoxor.d",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "amoand.d",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "amoor.d",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "amomin.d",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "amomax.d",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "amominu.d",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "amomaxu.d",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "flw",
        SyntacticDependency {
            sources: Some(&[RegType::Address]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "fsw",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: None,
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "fmadd.s",
        SyntacticDependency {
            sources: Some(&[
                RegType::Standard,
                RegType::Standard,
                RegType::Standard,
                RegType::Frm,
            ]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::OF, AccCsr::UF, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fmsub.s",
        SyntacticDependency {
            sources: Some(&[
                RegType::Standard,
                RegType::Standard,
                RegType::Standard,
                RegType::Frm,
            ]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::OF, AccCsr::UF, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fnmsub.s",
        SyntacticDependency {
            sources: Some(&[
                RegType::Standard,
                RegType::Standard,
                RegType::Standard,
                RegType::Frm,
            ]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::OF, AccCsr::UF, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fnmadd.s",
        SyntacticDependency {
            sources: Some(&[
                RegType::Standard,
                RegType::Standard,
                RegType::Standard,
                RegType::Frm,
            ]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::OF, AccCsr::UF, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fadd.s",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard, RegType::Frm]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::OF, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fsub.s",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard, RegType::Frm]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::OF, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fmul.s",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard, RegType::Frm]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::OF, AccCsr::UF, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fdiv.s",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard, RegType::Frm]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::DZ, AccCsr::OF, AccCsr::UF, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fsqrt.s",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Frm]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fsgnj.s",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "fsgnjn.s",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "fsgnjx.s",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "fmin.s",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV]),
            dependency_flags: (false, true),
        },
    ),
    (
        "fmax.s",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV]),
            dependency_flags: (false, true),
        },
    ),
    (
        "fcvt.w.s",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Frm]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fcvt.wu.s",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Frm]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fmv.x.w",
        SyntacticDependency {
            sources: Some(&[RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "feq.s",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV]),
            dependency_flags: (false, true),
        },
    ),
    (
        "flt.s",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV]),
            dependency_flags: (false, true),
        },
    ),
    (
        "fle.s",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV]),
            dependency_flags: (false, true),
        },
    ),
    (
        "fclass.s",
        SyntacticDependency {
            sources: Some(&[RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "fcvt.s.w",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Frm]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fcvt.s.wu",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Frm]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fmv.w.x",
        SyntacticDependency {
            sources: Some(&[RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "fcvt.l.s",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Frm]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fcvt.lu.s",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Frm]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fcvt.s.l",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Frm]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fcvt.s.lu",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Frm]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fld",
        SyntacticDependency {
            sources: Some(&[RegType::Address]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "fsd",
        SyntacticDependency {
            sources: Some(&[RegType::Address, RegType::Data]),
            destinations: None,
            accumulating_csrs: None,
            dependency_flags: (false, false),
        },
    ),
    (
        "fmadd.d",
        SyntacticDependency {
            sources: Some(&[
                RegType::Standard,
                RegType::Standard,
                RegType::Standard,
                RegType::Frm,
            ]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::OF, AccCsr::UF, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fmsub.d",
        SyntacticDependency {
            sources: Some(&[
                RegType::Standard,
                RegType::Standard,
                RegType::Standard,
                RegType::Frm,
            ]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::OF, AccCsr::UF, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fnmsub.d",
        SyntacticDependency {
            sources: Some(&[
                RegType::Standard,
                RegType::Standard,
                RegType::Standard,
                RegType::Frm,
            ]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::OF, AccCsr::UF, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fnmadd.d",
        SyntacticDependency {
            sources: Some(&[
                RegType::Standard,
                RegType::Standard,
                RegType::Standard,
                RegType::Frm,
            ]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::OF, AccCsr::UF, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fadd.d",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard, RegType::Frm]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::OF, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fsub.d",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard, RegType::Frm]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::OF, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fmul.d",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard, RegType::Frm]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::OF, AccCsr::UF, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fdiv.d",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard, RegType::Frm]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::DZ, AccCsr::OF, AccCsr::UF, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fsqrt.d",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Frm]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fsgnj.d",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "fsgnjn.d",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "fsgnjx.d",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "fmin.d",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV]),
            dependency_flags: (false, true),
        },
    ),
    (
        "fmax.d",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV]),
            dependency_flags: (false, true),
        },
    ),
    (
        "fcvt.s.d",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Frm]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::OF, AccCsr::UF, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fcvt.d.s",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV]),
            dependency_flags: (false, true),
        },
    ),
    (
        "feq.d",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV]),
            dependency_flags: (false, true),
        },
    ),
    (
        "flt.d",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV]),
            dependency_flags: (false, true),
        },
    ),
    (
        "fle.d",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV]),
            dependency_flags: (false, true),
        },
    ),
    (
        "fclass.d",
        SyntacticDependency {
            sources: Some(&[RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "fcvt.w.d",
        SyntacticDependency {
            sources: Some(&[RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fcvt.wu.d",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Frm]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fcvt.d.w",
        SyntacticDependency {
            sources: Some(&[RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "fcvt.d.wu",
        SyntacticDependency {
            sources: Some(&[RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "fcvt.l.d",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Frm]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fcvt.lu.d",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Frm]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NV, AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fmv.x.d",
        SyntacticDependency {
            sources: Some(&[RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
    (
        "fcvt.d.l",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Frm]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fcvt.d.lu",
        SyntacticDependency {
            sources: Some(&[RegType::Standard, RegType::Frm]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: Some(&[AccCsr::NX]),
            dependency_flags: (true, true),
        },
    ),
    (
        "fmv.d.x",
        SyntacticDependency {
            sources: Some(&[RegType::Standard]),
            destinations: Some(&[RegType::Standard]),
            accumulating_csrs: None,
            dependency_flags: (false, true),
        },
    ),
];

pub fn syntactic_dependency(mnemonic: &str) -> Option<&'static SyntacticDependency> {
    SYNTACTIC_DEP_PROPAGUATION
        .iter()
        .find_map(|(name, dep)| (*name == mnemonic).then_some(dep))
}
