# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


from enum import IntEnum, Enum, auto

##
# Architectural constants
##

ILEN = 4
WORD_SIZE = 4
DWORD_SIZE = 8
# Without the C extention (32 bit instructions), with C (16 bit) it falls to 1
# bits
BYTES_ALLIGN_4 = 2


# Important: the values must match the ones in the RISC-V specification,
# so do not use auto and make sure to skip the reserved values
class ExceptionCause(IntEnum):
    ID_INSTR_ADDR_MISALIGNED = 0
    ID_INSTR_ACCESS_FAULT = 1
    ID_ILLEGAL_INSTRUCTION = 2
    ID_BREAKPOINT = 3
    ID_LOAD_ADDR_MISALIGNED = 4
    ID_LOAD_ACCESS_FAULT = 5
    ID_STORE_AMO_ADDR_MISALIGNED = 6
    ID_STORE_AMO_ACCESS_FAULT = 7
    ID_ENVIRONMENT_CALL_FROM_U_MODE = 8
    ID_ENVIRONMENT_CALL_FROM_S_MODE = 9
    ID_ENVIRONMENT_CALL_FROM_M_MODE = 11
    ID_INSTRUCTION_PAGE_FAULT = 12
    ID_LOAD_PAGE_FAULT = 13
    ID_STORE_AMO_PAGE_FAULT = 15


###
# Some enums and constants
###


# We do not support hypervisor mode atm, as no open source design implements it
# afawk to date.
class PrivLvl(IntEnum):
    User = 0
    Supervisor = 1
    Hypervisor = 2
    Machine = 3


class FpuState(IntEnum):
    Off = 0
    Init = 1
    Clean = 2
    Dirty = 3


# Non-Reserved rounding modes
ROUNDING_MODES = [
    0,  # RNE, Round to Nearest, ties to Even
    1,  # RTZ, Round towards Zero
    2,  # RDN, Round Down
    3,  # RUP, Round Up
    4,  # RMM, Round to Nearest, ties to Max Magnitude
    # 7,  # select dynamic rounding mode from frm csr FIXME add this
]


class FenceOrdering(Enum):
    """Fence ordering, written in the form prior_sequential, no IO support yet
    format is fm [31-28] PI PO PR PW SI SO SR SW
    fm is 0000, and 1000 for tso
    """

    RW_RW = 0b0000_0_0_1_1_0_0_1_1
    TSO = 0b1000_0_0_1_1_0_0_1_1
    RW_W = 0b0000_0_0_1_1_0_0_0_1
    R_RW = 0b0000_0_0_1_0_0_0_1_1
    R_R = 0b0000_0_0_1_0_0_0_1_0
    W_W = 0b0000_0_0_0_1_0_0_0_1


class Ordering(Enum):
    "Store ordering bits in tuple (aq, rl)"

    NoOrd = (False, False)
    Rel = (False, True)
    Acq = (True, False)
    Seq = (True, True)
