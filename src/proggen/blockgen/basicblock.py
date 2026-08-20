# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


# This script is responsible for generating the basic blocks
from typing import TYPE_CHECKING
from proggen.instrgen import *
from proggen.pickinstr import *
from proggen.util import alloc_next_bb_addr, create_instr
from params import BRANCH_TAKEN_PROBA, BASIC_BLOCK_MIN_SPACE
from instgen import INSTRS_BY_ISA_CLASS, ISAInstrClass, is_last_bb_instr
from riscv import ILEN
from states import IntRegState

if TYPE_CHECKING:
    from states import FuzzerState, CoreState
    from params import TestParams


def gen_basicblock(
    fuzzerstate: "FuzzerState",
    corestate: "CoreState",
    test_params: "TestParams",
):
    """
    This function generates a single basic block. Instructions are added to the
    basic block as long as there is enough space, or if we randomly select a
    control flow instruction. If there is not enough space left, we generate a
    control flow instruction as fast as posisble.

    We need at least BASIC_BLOCK_MIN_SPACE, beacuse we need to chnage the state
    of a single register to CONSUMED if we need to use a JALR instruction during
    an emergency situation, which need 5 instruction to bring the register to
    maturity, and 1 for the jump itself.

    Returns:
        True if the basic block was successfully generated
    """
    curr_alloc_cursor = corestate.get_curr_bb_start_addr() + BASIC_BLOCK_MIN_SPACE
    curr_isa_class = None  # This is used in case there is only space for control flow
    is_block_terminated = False

    # TODO curr_alloc_cursor = curr_addr + BASIC_BLOCK_MIN_SPACE
    while (
        fuzzerstate.memstate.get_available_contig_space(curr_alloc_cursor) - 4
        > BASIC_BLOCK_MIN_SPACE
    ):
        curr_addr = corestate.get_current_addr()
        # Get the next instruction class
        curr_isa_class = gen_next_isainstrclass(fuzzerstate, corestate, test_params)

        # Decide on branch side
        if curr_isa_class == ISAInstrClass.BRANCH:
            corestate.curr_branch_taken = test_params.prng.random() < BRANCH_TAKEN_PROBA

        # Generate next bb addr if the instruction will terminate the block
        if is_last_bb_instr(
            curr_isa_class,
            corestate.curr_branch_taken,
            corestate.hartstate,
        ):
            # Create space for the next basic block.
            is_block_terminated = alloc_next_bb_addr(
                fuzzerstate, corestate, curr_isa_class, curr_addr
            )
            if not is_block_terminated:
                return False

        # Generate instruction
        late_block_termination, new_instrobjs = _gen_instruction(
            fuzzerstate, corestate, test_params, curr_isa_class, curr_addr
        )
        if not new_instrobjs:
            return False
        if late_block_termination:
            is_block_terminated = True

        # Update the program view
        corestate.add_instr(new_instrobjs)

        # return on termination
        if is_block_terminated:
            return True
        # Update memory and generate next instruction
        instr_mem_size = ILEN * len(new_instrobjs)
        # FIXME we should only allocate only what we need, and check for space,
        # so use curr addr instead of curr_alloc_cursor
        fuzzerstate.memstate.alloc_mem_range(curr_alloc_cursor, instr_mem_size)
        curr_alloc_cursor += instr_mem_size
        if __debug__:
            assert instr_mem_size < BASIC_BLOCK_MIN_SPACE  # NO_COMPRESSED

    is_block_terminated = _gen_urgent_cf_instruction(
        corestate, fuzzerstate, test_params
    )
    return is_block_terminated


def _gen_urgent_cf_instruction(
    corestate: "CoreState", fuzzerstate: "FuzzerState", test_params: "TestParams"
) -> bool:
    """
    Generates a control flow instruction as fast as possible
    """
    # TODO if a reg is consumed, we can add exceptions and CFI. Since the memory
    # is 20 bits anyways, a JAL is sufficient and we do not need the emergency
    # steering of some regs, which would save the minimal space for a BB
    # TODO, we can actually remove e.g. jalr if no consumed reg is present
    cfi_instr = [ISAInstrClass.JAL, ISAInstrClass.JALR, ISAInstrClass.BRANCH]
    curr_isa_class = test_params.prng.choice(cfi_instr)
    curr_addr = corestate.get_current_addr()

    if curr_isa_class == ISAInstrClass.JALR:
        instrs = bring_some_reg_to_state(
            IntRegState.CONSUMED, corestate, test_params.is_rv64
        )
        corestate.add_instr(instrs)
        curr_addr = corestate.get_current_addr()  # NO_COMPRESSED
    elif curr_isa_class == ISAInstrClass.BRANCH:
        corestate.curr_branch_taken = True

    next_addr_gen_success = alloc_next_bb_addr(
        fuzzerstate, corestate, curr_isa_class, curr_addr
    )
    if next_addr_gen_success == False:
        return False

    instr_strs = INSTRS_BY_ISA_CLASS[curr_isa_class]
    instr_str = test_params.prng.choice(instr_strs)
    corestate.add_instr(
        create_instr(
            instr_str,
            corestate,
            fuzzerstate,
            test_params.prng,
            curr_addr,
            test_params.is_rv64,
        )
    )

    return True


def _gen_instruction(
    fuzzerstate: "FuzzerState",
    corestate: "CoreState",
    test_params: "TestParams",
    curr_isa_class: ISAInstrClass,
    curr_addr: int,
) -> tuple[bool | None, list | None]:
    """Generates the next instruction in the current basic block. Handles both
    stateless instruction like additions, and instruction that depend on the
    current CPU state
    """
    late_block_termination = False
    match curr_isa_class:
        case ISAInstrClass.DESCEND_PRV:
            new_instrobjs = [gen_priv_descent_instr(corestate)]
            if new_instrobjs[0].will_trap:
                fuzzerstate.interrupt_cause["mret"] += 1
        case ISAInstrClass.EXCEPTION:
            new_instrobjs = [gen_exception_instr(corestate, test_params)]
        case ISAInstrClass.REGFSM:
            new_instrobjs = create_regfsm_instrobjs(
                corestate, test_params.is_rv64, test_params.prng
            )
        case ISAInstrClass.FPUFSM:
            new_instrobjs = gen_fpufsm_instrs(corestate, test_params)
        case ISAInstrClass.TVECFSM:
            new_instrobjs = [gen_tvecfill_instr(corestate, test_params)]
        case ISAInstrClass.EPCFSM:
            new_instrobjs = [gen_epcfill_instr(corestate, test_params)]
        case ISAInstrClass.RANDOM_CSR:
            new_instrobjs = [gen_random_csr_op(corestate, test_params)]
        case ISAInstrClass.MACHINE_CSR:
            new_instrobjs, will_trap = gen_mcsr_instr(corestate, test_params)
            if will_trap:
                # Statistics
                fuzzerstate.interrupt_cause["enable"] += 1
                if "smdbltrp" in test_params.design_extentions:
                    corestate.hartstate.mstatus_mdt = True
                late_block_termination = alloc_next_bb_addr(
                    fuzzerstate, corestate, curr_isa_class, curr_addr
                )
                if not late_block_termination:
                    return None, None
        case ISAInstrClass.WFI_TRAP:
            new_instrobjs = gen_waiting_instr_trap(fuzzerstate, corestate, test_params)
            # Statistics
            fuzzerstate.interrupt_cause["normal"] += 1
            fuzzerstate.wait_type["trap"] += 1
        case ISAInstrClass.WFI_NOTRAP:
            new_instrobjs = gen_waiting_instr_notrap(
                fuzzerstate, corestate, test_params
            )
            # Statistics
            fuzzerstate.wait_type["notrap"] += 1
        case ISAInstrClass.SEND_IPI:
            # Statistics
            fuzzerstate.n_ipi_sent += 1
            new_instrobjs = send_ipi(
                fuzzerstate, corestate, test_params.is_rv64, test_params.prng
            )
        case ISAInstrClass.SEND_LOCAL_INTERRUPT:
            new_instrobjs = send_local_interrupt(fuzzerstate, corestate, test_params)
            # Statistics
            fuzzerstate.n_local_interrupts_sent += 1
        case ISAInstrClass.CLEAR_INTERRUPT:
            new_instrobjs = clear_pending_interrupt(corestate, test_params.is_rv64)
        case ISAInstrClass.CLEAR_MDT:
            new_instrobjs = clear_mstatus_mdt(corestate, test_params.is_rv64)
        case ISAInstrClass.FREE_POLLUTED:
            new_instrobjs = freeing_instr(corestate, test_params)
        case ISAInstrClass.RESET_MCM:
            new_instrobjs = goto_mcm_reset_handler(
                fuzzerstate, corestate, test_params.is_rv64, test_params.prng
            )
        case ISAInstrClass.CREATE_ADDR_DEP:
            new_instrobjs = create_syntactic_dep(corestate, test_params.prng)
        case ISAInstrClass.WRITE_INSTR:
            new_instrobjs = write_foreign_instr(test_params, fuzzerstate, corestate)
        case ISAInstrClass.WAIT_FOR_INSTR:
            new_instrobjs = wait_for_instr(test_params, fuzzerstate, corestate)
        # Generate instruction which does not depend on the CPU state
        case _:
            instr_str = gen_next_instrstr_from_isaclass(
                curr_isa_class, test_params, corestate
            )
            new_instrobjs = create_instr(
                instr_str,
                corestate,
                fuzzerstate,
                test_params.prng,
                curr_addr,
                test_params.is_rv64,
            )

    return late_block_termination, new_instrobjs
