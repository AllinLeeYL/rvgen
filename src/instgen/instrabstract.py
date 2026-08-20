# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


# This script defines.

from .helper import (
    INSTRUCTION_IDS,
    PARAM_SIZES_BITS_32,
    PARAM_SIZES_BITS_64,
    PARAM_IS_SIGNED,
)
from riscv import *
from typing import Optional, TYPE_CHECKING
from abc import ABC, abstractmethod

if TYPE_CHECKING:
    from riscv import IntReg, FloatReg

###
# Abstract classes, cannot be instanciated
###


# TODO we could find in what ISAInstrclass the instruction is and store it as an
# attibute?
class RVInstr(ABC):
    def __init__(self, instr_str: str, iscompressed: bool = False):
        self.instr_str = instr_str
        self.iscompressed = iscompressed
        assert not iscompressed, "Compressed instructions are not yet supported."
        self.assert_authorized_instr_strs()

    authorized_instr_strs = ()

    def assert_authorized_instr_strs(self):
        """Check that it's not a wrong instruction id."""
        if __debug__:
            assert self.instr_str in self.__class__.authorized_instr_strs, (
                self.instr_str,
                self.__class__.authorized_instr_strs,
            )

    def get_inputregs(self) -> list[IntReg | FloatReg | CSR]:
        """Returns a list of input registers

        Warning, this function only handles all purpose regs
        """
        ret = []
        # All purpose regs
        rs1 = getattr(self, "rs1", None)
        rs2 = getattr(self, "rs2", None)
        frs1 = getattr(self, "frs1", None)
        frs2 = getattr(self, "frs2", None)
        frs3 = getattr(self, "frs3", None)
        for reg in (rs1, rs2, frs1, frs2, frs3):
            if reg is not None:
                ret.append(reg)
        return ret

    def get_outputregs(self) -> list[IntReg | FloatReg | CSR]:
        """Returns a list of output registers

        Warning, this function only handles all purpose regs
        """
        ret = []
        rd = getattr(self, "rd", None)
        frd = getattr(self, "frd", None)
        for reg in (rd, frd):
            if reg is not None:
                ret.append(reg)
        return ret

    @abstractmethod
    def gen_bytecode_int(self, is_spike_resolution: bool) -> int:
        """
        Parameters:
            is_spike_resolution: some instructions are treated differently
            between spike resolution and the subsequent actual simulation. This
            includes:
            - branches, which are resolved to unconditional jumps in spike.
            - placeholder instructions, which are not entangled with the data flow
                int: _description_
        """
        pass


# Any instruction with an immediate
class ImmInstr(RVInstr):
    def __init__(
        self,
        instr_str: str,
        imm: int,
        is_rv64: bool,
        iscompressed: bool = False,
    ):
        self.is_rv64 = is_rv64
        self.imm = imm
        super().__init__(instr_str, iscompressed)
        self.assert_imm_size()

    # Checks the immediate size.
    def assert_imm_size(self):
        if __debug__:
            assert hasattr(self, "imm")
            if self.is_rv64:
                curr_param_size = PARAM_SIZES_BITS_64[INSTRUCTION_IDS[self.instr_str]][
                    -1
                ]
            else:
                curr_param_size = PARAM_SIZES_BITS_32[INSTRUCTION_IDS[self.instr_str]][
                    -1
                ]
            if PARAM_IS_SIGNED[INSTRUCTION_IDS[self.instr_str]][-1]:
                assert self.imm >= -(1 << (curr_param_size - 1))
                assert self.imm < 1 << (curr_param_size - 1)
            else:
                assert self.imm >= 0
                assert self.imm < 1 << curr_param_size


class WrapperInstr(RVInstr):
    """This class is useful to get data of wrapped instruction"""

    def __init__(self, wrapped_instr: RVInstr):
        self.wrapped_instr = wrapped_instr
        super().__init__(wrapped_instr.instr_str)

    def get_inputregs(self) -> list[IntReg | FloatReg | CSR]:
        """Returns a list of input registers"""
        return self.wrapped_instr.get_inputregs()

    def get_outputregs(self) -> list[IntReg | FloatReg | CSR]:
        """Returns a list of output registers"""
        return self.wrapped_instr.get_outputregs()

    def gen_bytecode_int(self, is_spike_resolution: bool):
        return self.wrapped_instr.gen_bytecode_int(is_spike_resolution)


class TrapInstr(WrapperInstr):
    def __init__(
        self,
        instr: RVInstr,
        is_mtvec: bool,
        prev_priv: PrivLvl,
        producer_id: Optional[int] = None,
    ):
        self.is_mtvec = is_mtvec
        self.producer_id = producer_id
        self.prev_priv = prev_priv
        super().__init__(instr)


class CSRInstr(RVInstr):
    # static
    authorized_instr_strs = ("csrrw", "csrrs", "csrrc", "csrrwi", "csrrsi", "csrrci")

    # Checks the immediate size.
    def assert_csr_size(self):
        if __debug__:
            assert hasattr(self, "csr_id")
            assert self.csr_id >= 0
            assert self.csr_id < 1 << 12

    def __init__(self, instr_str: str, csr_id: int, iscompressed: bool = False):
        self.csr_id = csr_id
        self.assert_csr_size()
        super().__init__(instr_str, iscompressed)
