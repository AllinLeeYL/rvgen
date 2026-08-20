# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


import runparams
from proggen.instrgen import is_ops_available
from toleratebugs import (
    is_tolerate_kronos_fence,
    is_tolerate_picorv32_fence,
    is_forbid_vexriscv_csrs,
    is_tolerate_picorv32_missingmandatorycsrs,
    is_tolerate_picorv32_readnonimplcsr,
    is_tolerate_picorv32_writehpm,
    is_tolerate_picorv32_readhpm_nocsrrs,
)
from instgen import ISAInstrClass
from states import IntRegState, FloatRegState, SyncState
from riscv import PrivLvl, FpuState
from params import (
    ISAINSTRCLASS_INITIAL_BOOSTERS,
    NUM_MIN_OUTPUTS,
    NUM_MIN_INPUTS,
)
from random import Random
from copy import copy
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from states import CoreState, FuzzerState
    from params import TestParams

# This module helps picking an ISAInstrClass.
# This is the first step of generating a random instruction without a specific structure.

###
# Helper functions
###


# @param weights a list or as long as ISAInstrClass
# return a ISAInstrClass
# Do NOT @cache this function, as it is a random function.
def _gen_next_isainstrclass_from_weights(
    weights: dict[ISAInstrClass, float], prng: Random
) -> ISAInstrClass:
    ret = prng.choices(list(weights.keys()), weights=list(weights.values()))[0]
    assert weights[ret] != 0
    # assert ISAINSTRCLASS_INITIAL_BOOSTERS[ret] != 0
    return ret


def _filter_out_bugs(
    test_params: "TestParams", filtered_weights: dict[ISAInstrClass, float]
):
    """Handles know bugs in specific designs"""
    if "vexriscv" in test_params.design_name and is_forbid_vexriscv_csrs():
        # There is no notion of delegation if supervisor mode is not supported
        filtered_weights[ISAInstrClass.MACHINE_CSR] = 0
    if (
        "picorv32" in test_params.design_name
        or "vexriscv" in test_params.design_name
        and is_forbid_vexriscv_csrs()
    ):
        filtered_weights[ISAInstrClass.TVECFSM] = 0
    # For now, do not populate the mepc/sepc more than necessary TODO this is
    # too strict, take a look in your old code for MMU
    if (
        "picorv32" in test_params.design_name
        or "vexriscv" in test_params.design_name
        and is_forbid_vexriscv_csrs()
    ):
        filtered_weights[ISAInstrClass.EPCFSM] = 0
    # No exception if no exception is possible
    if "picorv32" in test_params.design_name:
        filtered_weights[ISAInstrClass.EXCEPTION] = 0
    if (
        "vexriscv" in test_params.design_name
        and is_forbid_vexriscv_csrs()
        or "picorv32" in test_params.design_name
        and not is_tolerate_picorv32_missingmandatorycsrs()
        and not is_tolerate_picorv32_readnonimplcsr()
        and not is_tolerate_picorv32_writehpm()
        and not is_tolerate_picorv32_readhpm_nocsrrs()
    ):
        filtered_weights[ISAInstrClass.RANDOM_CSR] = 0
    if (
        "kronos" in test_params.design_name
        and not is_tolerate_kronos_fence()
        or "picorv32" in test_params.design_name
        and not is_tolerate_picorv32_fence()
    ):
        filtered_weights[ISAInstrClass.FENCE] = 0


# @brief For now, the weights used for choosing instructions are fixed over time.
# This function filters the isainstrclass weights according to the capabilities of a given CPU
# FUTURE: Use coverage metrics or other kinds of scheduling.
# FUTURE: Use a Markov chain for executing floating point instructions in a row.
# @return a normalized dict of instruction classes supported by the CPU
# DO NOT @cache
def _get_isainstrclass_filtered_weights(
    test_params: "TestParams", corestate: "CoreState"
):
    hartstate = corestate.hartstate
    has_consumed_reg = corestate.intregpickstate.exists_reg_in_state(
        IntRegState.CONSUMED
    )
    ret_dict = copy(test_params.isapickweights)
    if not test_params.is_rv64:
        ret_dict[ISAInstrClass.ALU64] = 0
        ret_dict[ISAInstrClass.MULDIV64] = 0
        ret_dict[ISAInstrClass.AMO64] = 0
        ret_dict[ISAInstrClass.MEM64] = 0
        ret_dict[ISAInstrClass.FPU64] = 0
        ret_dict[ISAInstrClass.FPUD64] = 0

    if not test_params.design_has_fpu:
        ret_dict[ISAInstrClass.MEMFPU] = 0
        ret_dict[ISAInstrClass.FPU] = 0
        ret_dict[ISAInstrClass.FPU64] = 0
        ret_dict[ISAInstrClass.MEMFPUD] = 0
        ret_dict[ISAInstrClass.FPUD] = 0
        ret_dict[ISAInstrClass.FPUD64] = 0
        ret_dict[ISAInstrClass.FPUFSM] = 0

    if not test_params.design_has_fpud:
        ret_dict[ISAInstrClass.MEMFPUD] = 0
        ret_dict[ISAInstrClass.FPUD] = 0
        ret_dict[ISAInstrClass.FPUD64] = 0

    if not test_params.design_has_muldiv:
        ret_dict[ISAInstrClass.MULDIV] = 0
        ret_dict[ISAInstrClass.MULDIV64] = 0

    if not test_params.design_has_amo:
        ret_dict[ISAInstrClass.AMO] = 0
        ret_dict[ISAInstrClass.AMO64] = 0

    if corestate.hartstate.mstatus_fs == FpuState.Off:
        ret_dict[ISAInstrClass.MEMFPU] = 0
        ret_dict[ISAInstrClass.FPU] = 0
        ret_dict[ISAInstrClass.FPU64] = 0
        ret_dict[ISAInstrClass.MEMFPUD] = 0
        ret_dict[ISAInstrClass.FPUD] = 0
        ret_dict[ISAInstrClass.FPUD64] = 0

    if hartstate.privlvl != PrivLvl.Machine:
        ret_dict[ISAInstrClass.FPUFSM] = 0

    can_populate_mtvec_stvec = hartstate.privlvl == PrivLvl.Machine and (
        not hartstate.is_mtvec_populated
        or (not hartstate.is_stvec_populated and test_params.s_mode_support)
    )
    can_populate_stvec = hartstate.privlvl == PrivLvl.Supervisor and (
        not hartstate.is_stvec_populated and test_params.s_mode_support
    )

    if (not can_populate_mtvec_stvec) and (not can_populate_stvec):
        ret_dict[ISAInstrClass.TVECFSM] = 0

    can_populate_mepc_spec = hartstate.privlvl == PrivLvl.Machine and (
        not hartstate.is_mepc_populated
        or (not hartstate.is_sepc_populated and test_params.s_mode_support)
    )
    can_populate_sepc = (
        hartstate.privlvl == PrivLvl.Supervisor and not hartstate.is_sepc_populated
    )
    if (not can_populate_mepc_spec) and (not can_populate_sepc):
        ret_dict[ISAInstrClass.EPCFSM] = 0

    if hartstate.privlvl != PrivLvl.Machine:
        ret_dict[ISAInstrClass.MACHINE_CSR] = 0
    elif not is_ops_available(corestate, test_params):
        ret_dict[ISAInstrClass.MACHINE_CSR] = 0

    if not hartstate.is_ready_to_take_exception(test_params, has_consumed_reg):
        ret_dict[ISAInstrClass.EXCEPTION] = 0

    if not corestate.hartstate.is_ready_to_descend_privileges(
        test_params, has_consumed_reg
    ):
        ret_dict[ISAInstrClass.DESCEND_PRV] = 0

    if not hartstate.privlvl in (PrivLvl.Machine, PrivLvl.Supervisor):
        ret_dict[ISAInstrClass.RANDOM_CSR] = 0

    no_polluted_int_regs = not corestate.intregpickstate.exists_reg_in_state(
        IntRegState.POLLUTED
    )
    no_polluted_float_regs = (
        not corestate.floatregpickstate.exists_reg_in_state(FloatRegState.POLLUTED)
        or corestate.hartstate.mstatus_fs == FpuState.Off
    )
    if not runparams.FUZZ_MCM or (no_polluted_int_regs and no_polluted_float_regs):
        # no need to make extra deps if not fuzzing the MCM
        ret_dict[ISAInstrClass.FREE_POLLUTED] = 0

    if (
        not corestate.intregpickstate.exists_reg_in_state(IntRegState.ZEROED)
        or not runparams.USE_ADDR_REGS
    ):
        ret_dict[ISAInstrClass.CREATE_ADDR_DEP] = 0

    return ret_dict


def _filter_regfsm_weight(
    corestate: "CoreState", filtered_weights: dict[ISAInstrClass, float]
):
    """
    Sets the REGFSM weight to 0 if no register can be produced, consumed or
    relocated
    """
    # regfsm consumes an output register
    too_little_outputs = (
        corestate.intregpickstate.get_num_int_outputs() <= NUM_MIN_OUTPUTS
    )
    too_little_inputs = corestate.intregpickstate.get_num_int_input() <= NUM_MIN_INPUTS
    no_reg_beeing_produced = not (
        corestate.intregpickstate.exists_reg_in_state(IntRegState.PRODUCED0)
        or corestate.intregpickstate.exists_reg_in_state(IntRegState.PRODUCED1)
    )
    if (too_little_outputs or too_little_inputs) and no_reg_beeing_produced:
        filtered_weights[ISAInstrClass.REGFSM] = 0

    # send IPIs, local interrupts, clear interrupts and AMOS consume an INPUT
    # by polluting a reg
    # loads as well, but the need more fined grained control and are handled
    # at the instruction level, not class
    too_little_inputs = corestate.intregpickstate.get_num_int_input() <= NUM_MIN_INPUTS
    if too_little_inputs:
        filtered_weights[ISAInstrClass.SEND_IPI] = 0
        filtered_weights[ISAInstrClass.SEND_LOCAL_INTERRUPT] = 0
        filtered_weights[ISAInstrClass.CLEAR_INTERRUPT] = 0

    if too_little_inputs and runparams.FUZZ_MCM:
        filtered_weights[ISAInstrClass.AMO] = 0
        filtered_weights[ISAInstrClass.AMO64] = 0


def _filter_sensitive_instr_weights(
    corestate: "CoreState", filtered_weights: dict[ISAInstrClass, float]
):
    """
    Filters out the sensitive instructions considering whether there are
    available registers in the suitable state
    """
    if not corestate.intregpickstate.exists_reg_in_state(IntRegState.CONSUMED):
        filtered_weights[ISAInstrClass.JAL] = 0
        filtered_weights[ISAInstrClass.MACHINE_CSR] = 0
        filtered_weights[ISAInstrClass.TVECFSM] = 0
        filtered_weights[ISAInstrClass.EPCFSM] = 0
        filtered_weights[ISAInstrClass.JALR] = 0
        filtered_weights[ISAInstrClass.SEND_IPI] = 0
        # There is no need for a producer if address regs are used
        if not runparams.USE_ADDR_REGS:
            filtered_weights[ISAInstrClass.MEM] = 0
            filtered_weights[ISAInstrClass.MEM64] = 0
            filtered_weights[ISAInstrClass.MEMFPU] = 0
            filtered_weights[ISAInstrClass.MEMFPUD] = 0
            filtered_weights[ISAInstrClass.AMO] = 0
            filtered_weights[ISAInstrClass.AMO64] = 0


def _filter_corefsm_weights(
    fuzzerstate: "FuzzerState",
    corestate: "CoreState",
    test_params: "TestParams",
    filtered_weights: dict[ISAInstrClass, float],
):
    """Filters out the interrupt related actions based on the current state of
    the CPU and the fuzzer
    """
    hartstate = corestate.hartstate

    if test_params.num_harts == 1:
        filtered_weights[ISAInstrClass.SEND_IPI] = 0
        filtered_weights[ISAInstrClass.CLEAR_INTERRUPT] = 0
        filtered_weights[ISAInstrClass.WFI_TRAP] = 0
        filtered_weights[ISAInstrClass.WFI_NOTRAP] = 0
        return

    if corestate.interrupt_block:
        filtered_weights[ISAInstrClass.SEND_IPI] = 0
        filtered_weights[ISAInstrClass.SEND_LOCAL_INTERRUPT] = 0
        filtered_weights[ISAInstrClass.CLEAR_INTERRUPT] = 0
        filtered_weights[ISAInstrClass.WFI_TRAP] = 0
        filtered_weights[ISAInstrClass.WFI_NOTRAP] = 0
        filtered_weights[ISAInstrClass.TVECFSM] = 0
        filtered_weights[ISAInstrClass.EPCFSM] = 0
        filtered_weights[ISAInstrClass.EXCEPTION] = 0
        filtered_weights[ISAInstrClass.DESCEND_PRV] = 0
        filtered_weights[ISAInstrClass.MACHINE_CSR] = 0
        filtered_weights[ISAInstrClass.FPUFSM] = 0
        filtered_weights[ISAInstrClass.WAIT_FOR_INSTR] = 0
        filtered_weights[ISAInstrClass.WRITE_INSTR] = 0
        return

    will_trap = hartstate.mie_msie and (
        hartstate.privlvl != PrivLvl.Machine or hartstate.mstatus_mie
    )
    if will_trap and not hartstate.is_mtvec_populated:
        filtered_weights[ISAInstrClass.SEND_LOCAL_INTERRUPT] = 0
    if not will_trap and corestate.hartstate.privlvl != PrivLvl.Machine:
        filtered_weights[ISAInstrClass.SEND_LOCAL_INTERRUPT] = 0

    # if no cores are waitin, or we are waiting, we cannot send
    if not fuzzerstate.waiting_core_ids:
        filtered_weights[ISAInstrClass.SEND_IPI] = 0
    elif fuzzerstate.sync_states[corestate.hartid] == SyncState.INTERRUPT:
        filtered_weights[ISAInstrClass.SEND_IPI] = 0

    if hartstate.privlvl != PrivLvl.Machine:
        filtered_weights[ISAInstrClass.CLEAR_INTERRUPT] = 0
    elif not hartstate.mip_msip:
        filtered_weights[ISAInstrClass.CLEAR_INTERRUPT] = 0

    if not fuzzerstate.is_global_sync_state_free():
        filtered_weights[ISAInstrClass.WFI_TRAP] = 0
    elif not hartstate.is_mtvec_populated:
        filtered_weights[ISAInstrClass.WFI_TRAP] = 0
    elif len(fuzzerstate.waiting_core_ids) == test_params.num_harts - 1:
        filtered_weights[ISAInstrClass.WFI_TRAP] = 0
    elif not hartstate.mie_msie:
        filtered_weights[ISAInstrClass.WFI_TRAP] = 0
    elif hartstate.privlvl == PrivLvl.Machine and not hartstate.mstatus_mie:
        filtered_weights[ISAInstrClass.WFI_TRAP] = 0
    elif hartstate.mip_msip:
        # If there is a pending interrupt that causes a trap, it has trapped
        # already
        assert False

    if not fuzzerstate.is_global_sync_state_free():
        filtered_weights[ISAInstrClass.WFI_NOTRAP] = 0
    elif len(fuzzerstate.waiting_core_ids) == test_params.num_harts - 1:
        filtered_weights[ISAInstrClass.WFI_NOTRAP] = 0
    elif (
        hartstate.mie_msie
        and hartstate.privlvl == PrivLvl.Machine
        and hartstate.mstatus_mie
    ):
        filtered_weights[ISAInstrClass.WFI_NOTRAP] = 0
    elif hartstate.mie_msie and hartstate.privlvl != PrivLvl.Machine:
        filtered_weights[ISAInstrClass.WFI_NOTRAP] = 0

    exist_waiting_core = any(
        state == SyncState.WRITE_INSTR
        for hartid, state in fuzzerstate.sync_states.items()
        if hartid != corestate.hartid
    )
    if not exist_waiting_core:
        filtered_weights[ISAInstrClass.WRITE_INSTR] = 0
    if not fuzzerstate.is_global_sync_state_free():
        filtered_weights[ISAInstrClass.WAIT_FOR_INSTR] = 0


def _filter_out_memops(
    fuzzerstate: "FuzzerState",
    corestate: "CoreState",
    filtered_weights: dict[ISAInstrClass, float],
):
    """Filters out the memeory operation in the same block where
    we reached a new solving step.
    """
    if fuzzerstate.sync_states[corestate.hartid] == SyncState.MCM:
        filtered_weights[ISAInstrClass.MEM] = 0
        filtered_weights[ISAInstrClass.MEM64] = 0
        filtered_weights[ISAInstrClass.MEMFPU] = 0
        filtered_weights[ISAInstrClass.MEMFPUD] = 0
        filtered_weights[ISAInstrClass.AMO] = 0
        filtered_weights[ISAInstrClass.AMO64] = 0


###
# Exposed function
###


def gen_next_isainstrclass(
    fuzzerstate: "FuzzerState", corestate: "CoreState", test_params: "TestParams"
) -> ISAInstrClass:
    """Randomly chooses the next ISA instructin class to pick an instruction from.
    The choice is based off the current state of the core, such that the CF of
    the testcase generated stays valid
    """

    if corestate.has_reached_max_instr_num(
        test_params.nmax_instrs, len(corestate.basic_blocks[-1])
    ):
        return ISAInstrClass.JAL
    elif corestate.hartstate.mstatus_mdt:
        return ISAInstrClass.CLEAR_MDT
    elif (
        runparams.FUZZ_MCM
        and corestate.is_memop_limit_reached()
        and fuzzerstate.can_take_mcm_handler()
    ):
        return ISAInstrClass.RESET_MCM

    filtered_weights = _get_isainstrclass_filtered_weights(test_params, corestate)
    _filter_regfsm_weight(corestate, filtered_weights)
    _filter_sensitive_instr_weights(corestate, filtered_weights)
    _filter_corefsm_weights(fuzzerstate, corestate, test_params, filtered_weights)
    _filter_out_bugs(test_params, filtered_weights)
    _filter_out_memops(fuzzerstate, corestate, filtered_weights)

    # Normalize the weights
    weights_sum = sum(filtered_weights.values())
    if __debug__:
        assert weights_sum > 0, "the sum of filtered weights must be positive!"
    norm_factor = 1 / weights_sum
    for curr_key in filtered_weights:
        filtered_weights[curr_key] = filtered_weights[curr_key] * norm_factor

    return _gen_next_isainstrclass_from_weights(filtered_weights, test_params.prng)
