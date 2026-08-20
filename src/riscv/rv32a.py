# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


from .rvprotoinstrs import *

RV32A_OPCODE_AMO = 0b0101111

##
# Zalrsc
##


def rv32a_lrw(aq: bool, rl: bool, rd: int, rs1: int, rs2: int):
    funct3 = 0b010
    funct7 = 0b00010 << 2 | aq << 1 | rl
    return instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)


def rv32a_scw(aq: bool, rl: bool, rd: int, rs1: int, rs2: int):
    funct3 = 0b010
    funct7 = 0b00011 << 2 | aq << 1 | rl
    return instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)


##
# Zaamo
##


def rv32a_amoswapw(aq: bool, rl: bool, rd: int, rs1: int, rs2: int):
    funct3 = 0b010
    funct7 = 0b00001 << 2 | aq << 1 | rl
    return instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)


def rv32a_amoaddw(aq: bool, rl: bool, rd: int, rs1: int, rs2: int):
    funct3 = 0b010
    funct7 = 0b00000 << 2 | aq << 1 | rl
    return instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)


def rv32a_amoandw(aq: bool, rl: bool, rd: int, rs1: int, rs2: int):
    funct3 = 0b010
    funct7 = 0b01100 << 2 | aq << 1 | rl
    return instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)


def rv32a_amoorw(aq: bool, rl: bool, rd: int, rs1: int, rs2: int):
    funct3 = 0b010
    funct7 = 0b01000 << 2 | aq << 1 | rl
    return instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)


def rv32a_amoxorw(aq: bool, rl: bool, rd: int, rs1: int, rs2: int):
    funct3 = 0b010
    funct7 = 0b00100 << 2 | aq << 1 | rl
    return instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)


def rv32a_amomaxw(aq: bool, rl: bool, rd: int, rs1: int, rs2: int):
    funct3 = 0b010
    funct7 = 0b10100 << 2 | aq << 1 | rl
    return instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)


def rv32a_amomaxuw(aq: bool, rl: bool, rd: int, rs1: int, rs2: int):
    funct3 = 0b010
    funct7 = 0b11100 << 2 | aq << 1 | rl
    return instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)


def rv32a_amominw(aq: bool, rl: bool, rd: int, rs1: int, rs2: int):
    funct3 = 0b010
    funct7 = 0b10000 << 2 | aq << 1 | rl
    return instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)


def rv32a_amominuw(aq: bool, rl: bool, rd: int, rs1: int, rs2: int):
    funct3 = 0b010
    funct7 = 0b11000 << 2 | aq << 1 | rl
    return instruc_rtype(RV32A_OPCODE_AMO, rd, funct3, rs1, rs2, funct7)
