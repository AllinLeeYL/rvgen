# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only
from instgen import (
    ISAInstrClass,
    get_range_bits,
)
from params import BASIC_BLOCK_MIN_SPACE
from riscv import BYTES_ALLIGN_4, ILEN
from instgen import ISAInstrClass
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from states import FuzzerState, CoreState


def alloc_next_bb_addr(
    fuzzerstate: "FuzzerState",
    corestate: "CoreState",
    isa_class: ISAInstrClass,
    curr_addr: int,
    min_space: int = BASIC_BLOCK_MIN_SPACE,
) -> bool:
    """Finds a memory location with enough contiguous memory to accomodate a
    basic block, and allocates that address

    Args:
        isa_class (ISAInstrClass): range of the CF instruction
        curr_addr (int): address of the CF instruction

    Returns:
        bool: False is not address could be found
    """
    assert corestate.next_bb_addr is None
    # Allocate current instruction if called by the main loop, else we used the
    # reserved memory
    # We must select the next basic block address before the resolution
    instr_range = get_range_bits(isa_class)
    left_boundary = curr_addr - (1 << instr_range)
    right_boundary = curr_addr + (1 << instr_range)
    corestate.next_bb_addr = fuzzerstate.memstate.gen_random_free_addr(
        BYTES_ALLIGN_4, min_space, left_boundary, right_boundary
    )
    # If we could not find a new address where to place the next basic block,
    # then return and consider this stage complete.
    if corestate.next_bb_addr is None:
        corestate.next_bb_addr = None
        corestate.restore_previous_state()
        return False
    else:
        # Needed, or another core might get the address
        fuzzerstate.memstate.alloc_mem_range(corestate.get_next_bb_addr(), min_space)
        return True


def alloc_next_loopsegment(
    fuzzerstate: "FuzzerState",
    corestate: "CoreState",
    isa_class: ISAInstrClass,
    curr_addr: int,
    start_addr: int,
    max_loop_size: int,
    min_space: int = BASIC_BLOCK_MIN_SPACE,
) -> bool:
    """Finds a memory address that keeps a given address in range

    Args:
        isa_class (ISAInstrClass): range of the CF instruction
        start_addr (int): address that should stay in reach of the CF instr
        max_loop_size (int): Maximal size of the block

    Returns:
        bool: False is not address could be found
    """
    assert corestate.next_bb_addr is None
    # Allocate current instruction if called by the main loop, else we used the
    # reserved memory
    # We must select the next basic block address before the resolution
    instr_range = get_range_bits(isa_class)
    instr_range_branch = get_range_bits(ISAInstrClass.BRANCH)
    # Bound imposed by the current CF instruction
    imposed_left_boundary = curr_addr - (1 << instr_range) + ILEN
    imposed_right_boundary = curr_addr + (1 << instr_range) - ILEN
    # Bound imposed by the last branch instruction
    left_boundary = start_addr - (1 << instr_range_branch) + ILEN
    right_boundary = start_addr + ((1 << instr_range_branch) - max_loop_size) - ILEN
    left_boundary = max(left_boundary, imposed_left_boundary)
    right_boundary = min(right_boundary, imposed_right_boundary)
    corestate.next_bb_addr = fuzzerstate.memstate.gen_random_free_addr(
        BYTES_ALLIGN_4, min_space, left_boundary, right_boundary
    )
    # If we could not find a new address where to place the next basic block,
    # then return and consider this stage complete.
    if corestate.next_bb_addr is None:
        corestate.next_bb_addr = None
        corestate.restore_previous_state()
        return False
    else:
        # Needed, or another core might get the address
        fuzzerstate.memstate.alloc_mem_range(corestate.get_next_bb_addr(), min_space)
        return True
