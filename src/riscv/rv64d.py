# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


from .rvprotoinstrs import *

RV64D_OPCODE_FCVT = 0b1010011

# All functions return uint32_t


def rv64d_fcvtld(rd: int, rs1: int, rm: int):
    return instruc_rtype(RV64D_OPCODE_FCVT, rd, rm, rs1, 0b00010, 0b1100001)


def rv64d_fcvtlud(rd: int, rs1: int, rm: int):
    return instruc_rtype(RV64D_OPCODE_FCVT, rd, rm, rs1, 0b00011, 0b1100001)


def rv64d_fmvxd(rd: int, rs1: int):
    return instruc_rtype(RV64D_OPCODE_FCVT, rd, 0b000, rs1, 0b00000, 0b1110001)


def rv64d_fcvtdl(rd: int, rs1: int, rm: int):
    return instruc_rtype(RV64D_OPCODE_FCVT, rd, rm, rs1, 0b00010, 0b1101001)


def rv64d_fcvtdlu(rd: int, rs1: int, rm: int):
    return instruc_rtype(RV64D_OPCODE_FCVT, rd, rm, rs1, 0b00011, 0b1101001)


def rv64d_fmvdx(rd: int, rs1: int):
    return instruc_rtype(RV64D_OPCODE_FCVT, rd, 0b000, rs1, 0b00000, 0b1111001)
