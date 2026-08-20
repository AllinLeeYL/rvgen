# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


# This module is responsible for picking floating-point operations

from riscv import CSR, PrivLvl, ROUNDING_MODES, FpuState, IntReg
from params import FPU_STATE_BITS_REG
from instgen import CSRRegInstr, RegImmInstr
from typing import TYPE_CHECKING
from random import Random

if TYPE_CHECKING:
    from params import TestParams
    from states.corestate import CoreState


def _pick_rounding_mode(prng: Random):
    """Selects a random rounding mode"""
    return prng.sample(ROUNDING_MODES, 1)[0]


def create_rmswitch_instrobjs(corestate: "CoreState", test_params: "TestParams"):
    """Create a list of instructions to set a random rounding mode in the FPU. This
    is done either throufh the FRM CSR or the FCSR CSR. We first put the rounding
    mode to rinterm, and unset the flag bits

    Args:
        corestate ('FuzzerState'): The current state of the fuzzer

    Returns:
        list: A list of instructions to set the rounding mode in the FPU.
    """
    if __debug__:
        assert test_params.design_has_fpu
        assert corestate.hartstate.mstatus_fs != FpuState.Off
    new_rm = _pick_rounding_mode(test_params.prng)
    rinterm = corestate.intregpickstate.pick_int_outputreg_nonzero()
    rd = IntReg.zero  # FUTURE WARL
    use_frm_csr = test_params.prng.randint(0, 1)
    is_rv64 = test_params.is_rv64
    if use_frm_csr:
        return [
            RegImmInstr("addi", rinterm, IntReg.zero, new_rm, is_rv64),
            CSRRegInstr("csrrw", rd, rinterm, CSR.FRM),
        ]
    else:
        return [
            RegImmInstr("addi", rinterm, IntReg.zero, new_rm << 5, is_rv64),
            CSRRegInstr("csrrw", rd, rinterm, CSR.FCSR),
        ]


# @return a list of instrobjs
def gen_fpufsm_instrs(corestate: "CoreState", test_params: "TestParams") -> list:
    """
    Generate a list of instructions to manipulate the FPU (Floating Point Unit) state. This function
    generates instructions to either enable or disable the FPU, or to change the rounding mode of the
    FPU

    Args:
        corestate ('FuzzerState'): The current state of the fuzzer

    Returns:
        list: A list of instructions to manipulate the FPU state.
    """
    if __debug__:
        assert test_params.design_has_fpu
        assert corestate.hartstate.privlvl == PrivLvl.Machine

    ret = []
    is_fpu_activated = corestate.hartstate.mstatus_fs != FpuState.Off
    rd = IntReg.zero  # XiangShan
    if test_params.prng.random() < test_params.proba_keep_fpu_state:
        if is_fpu_activated:
            ret = [CSRRegInstr("csrrs", rd, FPU_STATE_BITS_REG[0], CSR.MSTATUS)]
        else:
            ret = [CSRRegInstr("csrrc", rd, FPU_STATE_BITS_REG[0], CSR.MSTATUS)]
    elif is_fpu_activated:
        do_change_rounding_mode = (
            test_params.prng.random() < test_params.proba_change_rm
        )
        if do_change_rounding_mode:
            ret = create_rmswitch_instrobjs(corestate, test_params)
        else:
            corestate.hartstate.set_mstatus_fs(FpuState.Off)
            ret = [CSRRegInstr("csrrc", rd, FPU_STATE_BITS_REG[0], CSR.MSTATUS)]
    else:
        corestate.hartstate.set_mstatus_fs(FpuState.Dirty)
        ret = [CSRRegInstr("csrrs", rd, FPU_STATE_BITS_REG[0], CSR.MSTATUS)]

    return ret
