// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only
//! Syntactic dependency metadata retained from the reference implementation.
//! These are operand roles, not a machine-state dependency analysis. The table
//! covers the reference's standard instructions; unknown/compressed forms return None.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegType {
    Data,
    Address,
    Standard,
    Csr,
    Frm,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccCsr {
    NV,
    OF,
    UF,
    NX,
    DZ,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryInstructionKind {
    IntLoad,
    FloatLoad,
    IntStore,
    FloatStore,
    Amo,
    AmoStore,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntacticDependency {
    pub sources: &'static [RegType],
    pub destinations: &'static [RegType],
    pub accumulating_csrs: &'static [AccCsr],
    /// Flags copied from the reference's dependency table, in its original order.
    pub dependency_flags: [bool; 2],
}
pub const LOAD_INSTR: &[MemoryInstructionKind] = &[
    MemoryInstructionKind::IntLoad,
    MemoryInstructionKind::FloatLoad,
];
pub const STORE_INSTR: &[MemoryInstructionKind] = &[
    MemoryInstructionKind::IntStore,
    MemoryInstructionKind::FloatStore,
];
pub const AMO_INSTR: &[MemoryInstructionKind] =
    &[MemoryInstructionKind::Amo, MemoryInstructionKind::AmoStore];
pub const MEMOP: &[MemoryInstructionKind] = &[
    MemoryInstructionKind::IntLoad,
    MemoryInstructionKind::FloatLoad,
    MemoryInstructionKind::IntStore,
    MemoryInstructionKind::FloatStore,
    MemoryInstructionKind::Amo,
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
pub fn syntactic_dependency_for(mnemonic: &str) -> Option<SyntacticDependency> {
    use AccCsr::*;
    use RegType::*;
    let (sources, destinations, accumulating_csrs, dependency_flags): (
        &'static [RegType],
        &'static [RegType],
        &'static [AccCsr],
        [bool; 2],
    ) = match mnemonic {
        "lui" | "auipc" | "jal" => (&[], &[Standard], &[], [false, false]),
        "jalr" => (&[Standard], &[Standard], &[], [false, false]),
        "beq" | "bne" | "blt" | "bge" | "bltu" | "bgeu" => {
            (&[Standard, Standard], &[], &[], [false, false])
        }
        "lb" | "lh" | "lw" | "lbu" | "lhu" | "lwu" | "ld" | "lr.w" | "lr.d" | "flw" | "fld" => {
            (&[Address], &[Standard], &[], [false, false])
        }
        "sb" | "sh" | "sw" | "sd" | "fsw" | "fsd" => (&[Address, Data], &[], &[], [false, false]),
        "addi" | "slti" | "sltiu" | "xori" | "ori" | "andi" | "slli" | "srli" | "srai"
        | "addiw" | "slliw" | "srliw" | "sraiw" | "fmv.x.w" | "fclass.s" | "fmv.w.x"
        | "fclass.d" | "fcvt.d.w" | "fcvt.d.wu" | "fmv.x.d" | "fmv.d.x" => {
            (&[Standard], &[Standard], &[], [false, true])
        }
        "add" | "sub" | "sll" | "slt" | "sltu" | "xor" | "srl" | "sra" | "or" | "and" | "addw"
        | "subw" | "sllw" | "srlw" | "sraw" | "mul" | "mulh" | "mulhsu" | "mulhu" | "div"
        | "divu" | "rem" | "remu" | "mulw" | "divw" | "divuw" | "remw" | "remuw" | "fsgnj.s"
        | "fsgnjn.s" | "fsgnjx.s" | "fsgnj.d" | "fsgnjn.d" | "fsgnjx.d" => {
            (&[Standard, Standard], &[Standard], &[], [false, true])
        }
        "fence" | "fence.i" | "ecall" | "ebreak" => (&[], &[], &[], [false, false]),
        "csrrw" | "csrrs" | "csrrc" => (&[Standard, Csr], &[Standard, Csr], &[], [true, true]),
        "csrrwi" | "csrrsi" | "csrrci" => (&[Csr], &[Standard, Csr], &[], [true, true]),
        "sc.w" | "sc.d" => (&[Address, Data], &[Standard], &[], [true, false]),
        "amoswap.w" | "amoadd.w" | "amoxor.w" | "amoand.w" | "amoor.w" | "amomin.w"
        | "amomax.w" | "amominu.w" | "amomaxu.w" | "amoswap.d" | "amoadd.d" | "amoxor.d"
        | "amoand.d" | "amoor.d" | "amomin.d" | "amomax.d" | "amominu.d" | "amomaxu.d" => {
            (&[Address, Data], &[Standard], &[], [false, false])
        }
        "fmadd.s" | "fmsub.s" | "fnmsub.s" | "fnmadd.s" | "fmadd.d" | "fmsub.d" | "fnmsub.d"
        | "fnmadd.d" => (
            &[Standard, Standard, Standard, Frm],
            &[Standard],
            &[NV, OF, UF, NX],
            [true, true],
        ),
        "fadd.s" | "fsub.s" | "fadd.d" | "fsub.d" => (
            &[Standard, Standard, Frm],
            &[Standard],
            &[NV, OF, NX],
            [true, true],
        ),
        "fmul.s" | "fmul.d" => (
            &[Standard, Standard, Frm],
            &[Standard],
            &[NV, OF, UF, NX],
            [true, true],
        ),
        "fdiv.s" | "fdiv.d" => (
            &[Standard, Standard, Frm],
            &[Standard],
            &[NV, DZ, OF, UF, NX],
            [true, true],
        ),
        "fsqrt.s" | "fcvt.w.s" | "fcvt.wu.s" | "fcvt.l.s" | "fcvt.lu.s" | "fsqrt.d"
        | "fcvt.wu.d" | "fcvt.l.d" | "fcvt.lu.d" => {
            (&[Standard, Frm], &[Standard], &[NV, NX], [true, true])
        }
        "fmin.s" | "fmax.s" | "feq.s" | "flt.s" | "fle.s" | "fmin.d" | "fmax.d" | "fcvt.d.s"
        | "feq.d" | "flt.d" | "fle.d" => (&[Standard, Standard], &[Standard], &[NV], [false, true]),
        "fcvt.s.w" | "fcvt.s.wu" | "fcvt.s.l" | "fcvt.s.lu" | "fcvt.d.l" | "fcvt.d.lu" => {
            (&[Standard, Frm], &[Standard], &[NX], [true, true])
        }
        "fcvt.s.d" => (
            &[Standard, Frm],
            &[Standard],
            &[NV, OF, UF, NX],
            [true, true],
        ),
        "fcvt.w.d" => (&[Standard], &[Standard], &[NV, NX], [true, true]),
        _ => return None,
    };
    Some(SyntacticDependency {
        sources,
        destinations,
        accumulating_csrs,
        dependency_flags,
    })
}
