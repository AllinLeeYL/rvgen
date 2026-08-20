# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


# This module is responsible for choosing the memory operation addresses and address registers.

from typing import TYPE_CHECKING
from enum import auto, IntEnum
import runparams

if TYPE_CHECKING:
    from states import FuzzerState


class MemaddrPickPolicy(IntEnum):
    """defines the address type one can select for a memory operation"""

    # proba weight to take any store location.
    MEM_ANY_STORELOC = auto()
    # proba weight to take any register and any authorized address.
    MEM_ANY = auto()


# Helper function for the basic blocks
def _is_instrstr_load(instr_str: str):
    # AMO are both load and store so store
    return instr_str in ("lb", "lh", "lhu", "lw", "lwu", "flw", "ld", "fld", "lbu")


# Helper function for the basic blocks
def _get_alignment_bits(instr_str: str):
    if instr_str in ("sb", "lb", "lbu"):
        return 0
    elif instr_str in ("sh", "lh", "lhu"):
        return 1
    elif instr_str in ("sw", "lw", "lwu", "flw", "fsw") or ".w" in instr_str:
        return 2
    elif instr_str in ("sd", "ld", "fld", "fsd") or ".d" in instr_str:
        return 3
    else:
        raise ValueError(f"Unexpected memory instruction string: `{instr_str}`")


def pick_memop_addr(fuzzerstate: "FuzzerState", instr_str: str, n_cores: int) -> int:
    """Chooses the target address of a given memory operartion"""
    is_curr_load = _is_instrstr_load(instr_str)
    alignment_bits = _get_alignment_bits(instr_str)

    if not is_curr_load:
        # Loads can only use load addresses
        curr_pick_type = MemaddrPickPolicy.MEM_ANY_STORELOC
    else:
        # FIXME when using a single core, we can allow the store locs
        # If we fuzz the MCM, use store locs for max overlap, else use any locs
        if runparams.FUZZ_MCM and n_cores > 1:
            curr_pick_type = MemaddrPickPolicy.MEM_ANY_STORELOC
        else:
            curr_pick_type = MemaddrPickPolicy.MEM_ANY

    if curr_pick_type == MemaddrPickPolicy.MEM_ANY_STORELOC:
        # Pick a store location
        ret_addr = fuzzerstate.pick_store_location(alignment_bits)
        if not is_curr_load:
            fuzzerstate.last_store_addr = ret_addr
    elif curr_pick_type == MemaddrPickPolicy.MEM_ANY:
        # Pick any location
        ret_addr = fuzzerstate.memview_blacklist.gen_random_free_addr(
            alignment_bits,
            min_space=1 << alignment_bits,
            left_bound=0,
            right_bound=fuzzerstate.memview_blacklist.memsize,
        )
        assert ret_addr is not None
        fuzzerstate.memview_blacklist.alloc_mem_range(ret_addr, 1 << alignment_bits)
        if __debug__:
            assert ret_addr >= 0
            assert ret_addr + (1 << alignment_bits) < fuzzerstate.memstate.memsize
    else:
        raise NotImplementedError(
            f"Unimplemented MemaddrPickPolicy: `{curr_pick_type}`."
        )
    return ret_addr
