# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


from .rvprotoinstrs import *

RV32I_OPCODE_PRIVILEGED = 0b1110011

# All functions return uint32_t


def rvprivileged_sret():
    return instruc_rtype(RV32I_OPCODE_PRIVILEGED, 0, 0, 0, 0b00010, 0b0001000)


def rvprivileged_mret():
    return instruc_rtype(RV32I_OPCODE_PRIVILEGED, 0, 0, 0, 0b00010, 0b0011000)


def rvprivileged_wfi():
    return instruc_rtype(RV32I_OPCODE_PRIVILEGED, 0, 0, 0, 0b00101, 0b0001000)


def rvprivileged_sfence_vma(rs1: int, rs2: int):
    return instruc_rtype(RV32I_OPCODE_PRIVILEGED, 0, 0, rs1, rs2, 0b0001001)


def rvprivileged_sinval_vma(rs1: int, rs2: int):
    return instruc_rtype(RV32I_OPCODE_PRIVILEGED, 0, 0, rs1, rs2, 0b0001011)


def rvprivileged_sfence_w_inval():
    return instruc_rtype(RV32I_OPCODE_PRIVILEGED, 0, 0, 0, 0b00000, 0b0001100)


def rvprivileged_sfence_inval_ir():
    return instruc_rtype(RV32I_OPCODE_PRIVILEGED, 0, 0, 0, 0b00001, 0b0001100)
