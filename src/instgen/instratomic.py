# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


# This script defines.

from typing import Optional
from instgen.instrabstract import RVInstr
from riscv import *

###
# Atomic instructions
###

# Instr with imm and rd
AmoInstrs = [
    "lr.w",
    "amoswap.w",
    "amoadd.w",
    "amoxor.w",
    "amoand.w",
    "amoor.w",
    "amomin.w",
    "amomax.w",
    "amominu.w",
    "amomaxu.w",
    "lr.d",
    "amoswap.d",
    "amoadd.d",
    "amoxor.d",
    "amoand.d",
    "amoor.d",
    "amomin.d",
    "amomax.d",
    "amominu.d",
    "amomaxu.d",
]


class AmoInstr(RVInstr):
    authorized_instr_strs = AmoInstrs

    def __init__(
        self,
        instr_str: str,
        rd: IntReg,  # dest
        rs1: IntReg,  # addr
        rs2: IntReg,  # src
        ordering: Ordering,
        producer_id: Optional[int],
        iscompressed: bool = False,
        dest_addr: Optional[int] = None,
    ):
        self.rd = rd
        self.rs1 = rs1
        self.rs2 = rs2
        self.ordering = ordering
        self.producer_id = producer_id
        self.dest_addr = dest_addr
        super().__init__(instr_str, iscompressed)

    def gen_bytecode_int(self, is_spike_resolution: bool):
        aq = self.ordering.value[0]
        rl = self.ordering.value[1]
        match self.instr_str:
            # rv32a
            case "lr.w":
                return rv32a_lrw(aq, rl, self.rd.value, self.rs1.value, self.rs2.value)
            case "amoswap.w":
                return rv32a_amoswapw(
                    aq, rl, self.rd.value, self.rs1.value, self.rs2.value
                )
            case "amoadd.w":
                return rv32a_amoaddw(
                    aq, rl, self.rd.value, self.rs1.value, self.rs2.value
                )
            case "amoxor.w":
                return rv32a_amoxorw(
                    aq, rl, self.rd.value, self.rs1.value, self.rs2.value
                )
            case "amoand.w":
                return rv32a_amoandw(
                    aq, rl, self.rd.value, self.rs1.value, self.rs2.value
                )
            case "amoor.w":
                return rv32a_amoorw(
                    aq, rl, self.rd.value, self.rs1.value, self.rs2.value
                )
            case "amomin.w":
                return rv32a_amominw(
                    aq, rl, self.rd.value, self.rs1.value, self.rs2.value
                )
            case "amomax.w":
                return rv32a_amomaxw(
                    aq, rl, self.rd.value, self.rs1.value, self.rs2.value
                )
            case "amominu.w":
                return rv32a_amominuw(
                    aq, rl, self.rd.value, self.rs1.value, self.rs2.value
                )
            case "amomaxu.w":
                return rv32a_amomaxuw(
                    aq, rl, self.rd.value, self.rs1.value, self.rs2.value
                )
            # rv32a
            case "lr.d":
                return rv64a_lrd(aq, rl, self.rd.value, self.rs1.value, self.rs2.value)
            case "amoswap.d":
                return rv64a_amoswapd(
                    aq, rl, self.rd.value, self.rs1.value, self.rs2.value
                )
            case "amoadd.d":
                return rv64a_amoaddd(
                    aq, rl, self.rd.value, self.rs1.value, self.rs2.value
                )
            case "amoxor.d":
                return rv64a_amoxord(
                    aq, rl, self.rd.value, self.rs1.value, self.rs2.value
                )
            case "amoand.d":
                return rv64a_amoandd(
                    aq, rl, self.rd.value, self.rs1.value, self.rs2.value
                )
            case "amoor.d":
                return rv64a_amoord(
                    aq, rl, self.rd.value, self.rs1.value, self.rs2.value
                )
            case "amomin.d":
                return rv64a_amomind(
                    aq, rl, self.rd.value, self.rs1.value, self.rs2.value
                )
            case "amomax.d":
                return rv64a_amomaxd(
                    aq, rl, self.rd.value, self.rs1.value, self.rs2.value
                )
            case "amominu.d":
                return rv64a_amominud(
                    aq, rl, self.rd.value, self.rs1.value, self.rs2.value
                )
            case "amomaxu.d":
                return rv64a_amomaxud(
                    aq, rl, self.rd.value, self.rs1.value, self.rs2.value
                )
            case _:
                raise ValueError(f"Unexpected instruction string: `{self.instr_str}`.")


AmoStores = ["sc.w, sc.d"]


class AmoStore(AmoInstr):
    authorized_instr_strs = AmoInstrs

    def __init__(
        self,
        instr_str: str,
        rd: IntReg,
        rs1: IntReg,
        ordering: Ordering,
        iscompressed: bool = False,
    ):
        super().__init__(instr_str, rd, rs1, IntReg.zero, ordering, iscompressed)

    def gen_bytecode_int(self, is_spike_resolution: bool):
        aq = self.ordering.value[0]
        rl = self.ordering.value[1]
        match self.instr_str:
            # rv32a
            case "sc.w":
                return rv32a_scw(aq, rl, self.rd.value, self.rs1.value, self.rs2.value)
            # rv64a
            case "sc.d":
                return rv64a_scd(aq, rl, self.rd.value, self.rs1.value, self.rs2.value)
            case _:
                raise ValueError(f"Unexpected instruction string: `{self.instr_str}`.")
