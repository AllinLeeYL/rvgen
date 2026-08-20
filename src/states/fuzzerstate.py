# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only
"""
This module stores the system wide state of the fuzzer. Mainly, it generates
the final and initial bloc which are shared by all cores, and keeps track of the
current allocation states through the MemoryState classes
"""


from enum import Enum, auto
from typing import Optional, TYPE_CHECKING
from random import Random
from states.memstate import MemState
from params import MAX_NUM_STORE_LOCATIONS

if TYPE_CHECKING:
    from params import TestParams
    from instgen import RVInstr
    from riscv import IntReg
ALIGNMENT_BITS_MAX = 3  # quad RISC-V extensions not supported
BASE_ADDR = 0x80000000


class SyncState(Enum):
    FREE = auto()
    INTERRUPT = auto()
    MCM = auto()
    WRITE_INSTR = auto()


class FuzzerState:
    """Class to keep track to some fuzzer states which are shared between cores"""

    def __init__(
        self,
        test_params: "TestParams",
    ):
        self.prng: Random = test_params.prng

        self.initial_reg_data_addr = -1
        self.initial_reg_data_content: list[int] = []
        self.init_bb_instrs: list[RVInstr] = []
        self.init_bb_base_addr: int = 0
        self.all_addr_regs: list[IntReg] = []

        self.memstate = MemState(test_params.memsize, test_params.prng)
        self.memview_blacklist = MemState(test_params.memsize, test_params.prng)

        self.num_store_locs: int = test_params.prng.randint(1, MAX_NUM_STORE_LOCATIONS)
        self.store_locations: list[int] = []
        self.last_store_addr: Optional[int] = None
        self.location_weights: list[float] = []

        self.final_bb_instrs: list[RVInstr] = []
        self.final_bb_base_addr: Optional[int] = -1

        self.stress_bb: list[RVInstr] = []

        self.mcm_state_rst_instrs: list[RVInstr] = []
        self.mcm_state_rst_base_addr: int = test_params.memsize

        # Context setter. Currently, we removed the context setter. In the,
        # future, when we implement the reduction, we will allocate it outside
        # the fuzzable memory range, like the mcm state reset handler
        self.ctxs_instrs: list[RVInstr] = []
        self.ctxs_base_addr: Optional[int] = -1

        self.waiting_core_ids: list[int] = []
        self.mtimer_reset_idx: Optional[int] = None

        # strictly increasing for uniqueness
        self.curr_store_val = 1

        self.sync_states: dict[int, SyncState] = {
            hartid: SyncState.FREE for hartid in test_params.hartids
        }

        # the generation sequences for the address and instruction of the WRITE_INSTR
        # scenario
        self.instr_write_gen_sequences: Optional[tuple[int, int, int]] = None

        # Statistics
        self.n_ipi_sent: int = 0
        self.n_local_interrupts_sent: int = 0
        self.interrupt_cause: dict[str, int] = {
            "normal": 0,
            "enable": 0,
            "mret": 0,
        }
        self.landing_type: dict[str, int] = {
            "soft": 0,
            "hard": 0,
        }
        self.wait_type: dict[str, int] = {
            "trap": 0,
            "notrap": 0,
        }

    def get_final_bb_base_addr(self) -> int:
        """returns the address of the final basic block"""
        assert self.final_bb_base_addr is not None
        assert self.final_bb_base_addr >= 0
        return self.final_bb_base_addr

    def set_init_instrs(self, new_instrobjs: list):
        """Adds instruction(s) to the initial block instruction list"""
        self.init_bb_instrs = new_instrobjs

    def is_global_sync_state_free(self):
        """The global sync state is free only if all hart are free"""
        return all(state == SyncState.FREE for state in self.sync_states.values())

    def can_take_mcm_handler(self):
        """a core can go to the reset handler if no core is waiting, or if
        a core is waiting in an MCM handler
        """
        allowed_states = (SyncState.FREE, SyncState.MCM)
        allowed = all(state in allowed_states for state in self.sync_states.values())
        return allowed

    def init_store_locations(self):
        """Should be called once the first basic block is already allocated"""
        memstate = self.memstate
        blacklist_memstate = self.memview_blacklist
        for _ in range(self.num_store_locs):
            next_store_loc = memstate.gen_random_free_addr(
                ALIGNMENT_BITS_MAX, 1 << ALIGNMENT_BITS_MAX, 0, memstate.memsize
            )
            if next_store_loc is None:
                raise ValueError(
                    f"Could not find a next store location. You may want to increase the memory size (for the moment: {memstate.memsize:,} B)"
                )
            memstate.alloc_mem_range(next_store_loc, (1 << ALIGNMENT_BITS_MAX))
            # FIXME atomics, for now we blacklist all store locations
            blacklist_memstate.alloc_mem_range(
                next_store_loc, (1 << ALIGNMENT_BITS_MAX)
            )
            self.store_locations.append(next_store_loc)
            if __debug__:
                assert next_store_loc >= 0
                assert next_store_loc + (1 << ALIGNMENT_BITS_MAX) <= memstate.memsize
                assert next_store_loc % (1 << ALIGNMENT_BITS_MAX) == 0
        self.location_weights = [1.0] * self.num_store_locs
        # Remember the last store operation address
        self.last_store_addr = self.store_locations[0]

    def pick_store_location(self, alignment_bits: int):
        """Modifies the weights in place.

        Parameters:
            alignment_bits is equal to the requested size. This means we do not
            support misaligned mem reqs.

        Returns:
            the picked location, in addition to updating the state.
        """
        if __debug__:
            assert alignment_bits >= 0 and alignment_bits <= ALIGNMENT_BITS_MAX

        # We first pick a store location. If the alignment is smaller than this
        # size, then we choose uniformly inside the selected store location.
        picked_location_id = self.prng.choices(
            range(len(self.store_locations)), list(self.location_weights)
        )[0]
        picked_location = self.store_locations[picked_location_id]
        # Update the weights using a heuristic algorithm
        total_weight = sum(self.location_weights)
        if total_weight != 0:
            self.location_weights = [w / total_weight for w in self.location_weights]
        self.location_weights[picked_location_id] = 1

        if alignment_bits == ALIGNMENT_BITS_MAX:
            return picked_location
        else:
            # Choose uniformly a sub-location
            factor = 1 << (ALIGNMENT_BITS_MAX - alignment_bits)
            offset_in_location = self.prng.randrange(factor) * (1 << alignment_bits)
            return picked_location + offset_in_location
