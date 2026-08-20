# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


from .rvprotoinstrs import *

RV64A_OPCODE_AMO = 0b0101111

##
# Zalrsc
##


def rv64a_lrd(aq: bool, rl: bool, rd: int, rs1: int, rs2: int):
    funct3 = 0b011
    funct7 = 0b00010 << 2 | aq << 1 | rl
    return instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)


def rv64a_scd(aq: bool, rl: bool, rd: int, rs1: int, rs2: int):
    funct3 = 0b011
    funct7 = 0b00011 << 2 | aq << 1 | rl
    return instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)


##
# Zaamo
##


def rv64a_amoswapd(aq: bool, rl: bool, rd: int, rs1: int, rs2: int):
    funct3 = 0b011
    funct7 = 0b00001 << 2 | aq << 1 | rl
    return instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)


def rv64a_amoaddd(aq: bool, rl: bool, rd: int, rs1: int, rs2: int):
    funct3 = 0b011
    funct7 = 0b00000 << 2 | aq << 1 | rl
    return instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)


def rv64a_amoandd(aq: bool, rl: bool, rd: int, rs1: int, rs2: int):
    funct3 = 0b011
    funct7 = 0b01100 << 2 | aq << 1 | rl
    return instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)


def rv64a_amoord(aq: bool, rl: bool, rd: int, rs1: int, rs2: int):
    funct3 = 0b011
    funct7 = 0b01000 << 2 | aq << 1 | rl
    return instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)


def rv64a_amoxord(aq: bool, rl: bool, rd: int, rs1: int, rs2: int):
    funct3 = 0b011
    funct7 = 0b00100 << 2 | aq << 1 | rl
    return instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)


def rv64a_amomaxd(aq: bool, rl: bool, rd: int, rs1: int, rs2: int):
    funct3 = 0b011
    funct7 = 0b10100 << 2 | aq << 1 | rl
    return instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)


def rv64a_amomaxud(aq: bool, rl: bool, rd: int, rs1: int, rs2: int):
    funct3 = 0b011
    funct7 = 0b11100 << 2 | aq << 1 | rl
    return instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)


def rv64a_amomind(aq: bool, rl: bool, rd: int, rs1: int, rs2: int):
    funct3 = 0b011
    funct7 = 0b10000 << 2 | aq << 1 | rl
    return instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)


def rv64a_amominud(aq: bool, rl: bool, rd: int, rs1: int, rs2: int):
    funct3 = 0b011
    funct7 = 0b11000 << 2 | aq << 1 | rl
    return instruc_rtype(RV64A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
