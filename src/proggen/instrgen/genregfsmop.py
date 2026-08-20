# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


from states import IntRegState
from params import NUM_MIN_OUTPUTS, NUM_MIN_INPUTS, REG_FSM_WEIGHTS
from instgen.instrfuzzer import *
from typing import TYPE_CHECKING
from random import Random
import numpy as np

if TYPE_CHECKING:
    from ...states import CoreState


def _create_targeted_producer0_instrobj(corestate: "CoreState", is_rv64: bool):
    corestate.next_producer_id += 1
    rd = corestate.intregpickstate.pick_int_outputreg_nonzero(False)
    corestate.intregpickstate.set_producer_id(rd, corestate.next_producer_id)
    corestate.intregpickstate.set_regstate(rd, IntRegState.PRODUCED0)
    return [PlaceholderProducerInstr0(rd, corestate.next_producer_id, is_rv64)]


def _create_targeted_producer1_instrobj(corestate: "CoreState", is_rv64: bool):
    rd = corestate.intregpickstate.pick_reg_in_state(IntRegState.PRODUCED0)
    corestate.intregpickstate.set_regstate(rd, IntRegState.PRODUCED1)
    return [
        PlaceholderProducerInstr1(
            rd,
            corestate.intregpickstate.get_producer_id(rd),
            is_rv64,
        )
    ]


def _create_targeted_consumer_instrobj(corestate: "CoreState", is_rv64: bool):
    pickreg = corestate.intregpickstate
    # We want to create dependencies, therefore we choose not to accept x0
    rprod = pickreg.pick_reg_in_state(IntRegState.PRODUCED1)
    # Since this reg is also used as an output, we must free it manually if it
    # as previously zeroed
    rdep = pickreg.pick_int_inputreg_nonzero()
    if corestate.intregpickstate.get_regstate(rdep) == IntRegState.ZEROED:
        corestate.intregpickstate.set_regstate(rdep, IntRegState.FREE, force=True)
    rprod_id = pickreg.get_producer_id(rprod)
    # WARNING: We CANNOT throw a PRODUCEDX into the nature because its value
    # will change between spike and RTL.
    rd = rprod
    pickreg.set_regstate(rprod, IntRegState.CONSUMED)
    if is_rv64:
        # TODO, we may not need a preconsumer for the prod reg if we craft the
        # offset carefully
        return [
            PlaceholderPreConsumerInstr(rprod),
            PlaceholderPreConsumerInstr(rdep),
            PlaceholderConsumerInstr(rd, rdep, rprod, rprod_id),
        ]
    else:
        return [PlaceholderConsumerInstr(rd, rdep, rprod, rprod_id)]


def create_regfsm_instrobjs(corestate: "CoreState", is_rv64: bool, prng: Random):
    # Check which reg fsm operations are doable
    doable_fsm_ops = [0] * 3
    if (
        corestate.intregpickstate.get_num_int_outputs() > NUM_MIN_OUTPUTS
        and corestate.intregpickstate.get_num_int_input() > NUM_MIN_INPUTS
    ):
        doable_fsm_ops[0] = REG_FSM_WEIGHTS[0]

    if corestate.intregpickstate.exists_reg_in_state(IntRegState.PRODUCED0):
        doable_fsm_ops[1] = REG_FSM_WEIGHTS[1]

    if corestate.intregpickstate.exists_reg_in_state(IntRegState.PRODUCED1):
        doable_fsm_ops[2] = REG_FSM_WEIGHTS[2]

    choice = prng.choices(range(3), doable_fsm_ops, k=1)[0]

    if choice == 0:  # FREE -> PRODUCED0
        return _create_targeted_producer0_instrobj(corestate, is_rv64)
    elif choice == 1:  # PRODUCED0 -> PRODUCED1
        return _create_targeted_producer1_instrobj(corestate, is_rv64)
    elif choice == 2:  # PRODUCED1 -> FREE/CONSUMED
        return _create_targeted_consumer_instrobj(corestate, is_rv64)
    else:
        raise ValueError(f"Unexpected choice: `{choice}`.")


def bring_some_reg_to_state(
    req_state: int, corestate: "CoreState", is_rv64: bool
) -> list:
    """
    Brings a register to the requested state as fast as possible.

    Parameters:
        req_state: The requested state of the register.
        corestate: The current state of the fuzzer.
        is_rv64: architecture size
    """
    instrs = []
    if req_state == IntRegState.CONSUMED:
        if corestate.intregpickstate.exists_reg_in_state(IntRegState.CONSUMED):
            pass
        elif corestate.intregpickstate.exists_reg_in_state(IntRegState.PRODUCED1):
            instrs += _create_targeted_consumer_instrobj(corestate, is_rv64)
        elif corestate.intregpickstate.exists_reg_in_state(IntRegState.PRODUCED0):
            instrs += _create_targeted_producer1_instrobj(corestate, is_rv64)
            instrs += _create_targeted_consumer_instrobj(corestate, is_rv64)
        elif corestate.intregpickstate.get_num_int_outputs():
            instrs += _create_targeted_producer0_instrobj(corestate, is_rv64)
            instrs += _create_targeted_producer1_instrobj(corestate, is_rv64)
            instrs += _create_targeted_consumer_instrobj(corestate, is_rv64)
        else:
            raise ValueError("Unexpected state.")
    else:
        raise NotImplementedError()

    return instrs
