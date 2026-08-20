# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only
"""
corestates.py

This module store core-specific data. Its main goal are to create the hartstsate
class, keep track of the instruction meant to run on this core, and track some
data that is core-specific and useful during generation
"""


from random import Random
from typing import Optional, TYPE_CHECKING
from states.hartstate import HartState
from states.regstate import IntRegPickState, FloatRegPickState, IntRegState
from riscv import ILEN, IntReg, FloatReg
from params import (
    MIN_NUM_PICKABLE_INTREGS,
    MIN_NUM_PICKABLE_FLOATREGS,
    MAX_NUM_PICKABLE_INTREGS,
    MAX_NUM_PICKABLE_FLOATREGS,
)
import runparams

if TYPE_CHECKING:
    from params import TestParams
    from instgen import ISAInstrClass, RVInstr


class CoreState:
    """class used to keep track of important fuzzer imformation, which are
    propritary to one core, like the register states, the hart state, the
    current testcase instruction stream, ...
    """

    def __init__(
        self,
        test_params: "TestParams",
        hartid: int,
    ):
        self.hartid: int = hartid
        pickable_intregs: list[IntReg] = self.pick_allowed_intregs(test_params.prng)
        if test_params.design_has_fpu:
            pickable_floatregs: list[FloatReg] = self.pick_allowed_floatregs(
                test_params.prng
            )
        else:
            pickable_floatregs = []
        # States
        self.intregpickstate: IntRegPickState = IntRegPickState(
            pickable_intregs, test_params.prng
        )
        self.floatregpickstate: FloatRegPickState = FloatRegPickState(
            pickable_floatregs, test_params.prng
        )
        self.hartstate: HartState = HartState(test_params.design_has_fpu)
        self.saved_reg_states: list = []
        # Instructions, FIXME make a dictionary
        self.basic_blocks: list[list[RVInstr]] = []
        self.bb_start_addrs: list[int] = []
        # Generation helpers
        self.addr_reg_pairs: dict[int, tuple[IntReg, int]] = {}
        self.addr_regs: list[IntReg] = []
        self.curr_branch_taken: bool = False
        self.next_producer_id: int = 0  # strictly increasing for uniqueness
        self.curr_bb_start_addr: Optional[int] = 0  # Initial block starts at 0
        self.next_bb_addr: Optional[int] = None
        self.state_preserving_loop: bool = False
        self.interrupt_block: bool = False
        self.stateloop_cmp_reg: Optional[IntReg] = None
        self.curr_n_memops: int = 0
        self.step_memops_limit: int = runparams.MAX_N_CONSECUTIVE_MEMOPS
        self.mcm_reset_locs: list[tuple[int, int]] = []
        # Stats
        self.total_fuzzing_instrs: int = 0
        # Data for ultimate generation
        self.producer_id_to_tgtaddr: dict = {}
        self.interrupt_handlers: list[list[RVInstr]] = []
        self.interrupt_handlers_start_addr: list[int] = []
        self.clint_instr: list[tuple[int, int]] = []
        self.interrupt_locs: list[tuple[ISAInstrClass, int]] = []

        # Rocket has some inaccuracy in minstret because of ebreak and ecall.
        # Hence, we don't read instret after these 2 instructions.
        self.rocket_minsret_innacurate = False

    def pick_addr_regs(self, num_addr_regs: int, prng: Random):
        """Pick, from the set of unused register (registers not picked for
        fuzzing), some registers that will be initialized with an address for
        load and stores
        """
        all_regs = set(list(IntReg)[1 : MAX_NUM_PICKABLE_INTREGS + 1])
        used_regs = set(self.intregpickstate.pickable_intregs)
        unused_regs = all_regs - used_regs
        # ensure determinism across runs by using a list
        unused_regs_list = sorted(list(unused_regs), key=lambda x: x.value)
        self.addr_regs = prng.sample(unused_regs_list, num_addr_regs)

    def get_reg_to_addr_mapping(self) -> dict[IntReg, int]:
        """reverses the dictionary of address register such that register are
        the keys and addresses are the values. Used for the initial block,
        skips entries with offsets
        """
        ret: dict[IntReg, int] = {}
        for addr, (reg, offset) in self.addr_reg_pairs.items():
            if offset != 0:
                continue
            ret[reg] = addr
        return ret

    def enter_stateloop_gen(self):
        """Set some option used to generate stateloops"""
        self.init_new_bb()
        self.intregpickstate.enter_stateloop()
        self.floatregpickstate.enter_stateloop()

    def exit_stateloop_gen(self):
        """reset regstaes, etc when done with stateloop generation"""
        self.state_preserving_loop = False
        assert self.stateloop_cmp_reg is not None
        self.intregpickstate.set_regstate(self.stateloop_cmp_reg, IntRegState.FREE)
        self.stateloop_cmp_reg = None
        # Allow all registers to be picked freely again
        self.intregpickstate.exit_stateloop()
        self.floatregpickstate.exit_stateloop()

    def add_memop(self):
        """Adds a memory operation to the current memory operation count. Used
        to know when a MCM reset block should be generated
        """
        self.curr_n_memops += 1

    def reset_memop_count(self, prng: Random):
        """Reset the memop count, should be called when an MCM reset block is
        generated
        """
        # self.step_memops_limit = prng.randint(1, runparams.MAX_N_CONSECUTIVE_MEMOPS)
        self.step_memops_limit = runparams.MAX_N_CONSECUTIVE_MEMOPS
        self.curr_n_memops = 0

    def add_clint_instr(self, offset: int = 0):
        """Appends the coordinates of a clint instruction"""
        bb_id = len(self.basic_blocks) - 1
        instr_id = len(self.basic_blocks[bb_id]) + offset
        self.clint_instr.append((bb_id, instr_id))

    def pick_allowed_intregs(self, prng: Random):
        """Generates the list of allowed integer register for the current
        testcase, the last register is never x0 for convinience
        """
        intregs = list(IntReg)[:MAX_NUM_PICKABLE_INTREGS]
        num_pickable_intregs = prng.randint(
            MIN_NUM_PICKABLE_INTREGS, MAX_NUM_PICKABLE_INTREGS
        )
        allowed_intregs = prng.sample(intregs, num_pickable_intregs)
        return allowed_intregs

    def pick_allowed_floatregs(self, prng: Random):
        """Generates the list of allowed floating point register for the current
        testcase
        """
        floatregs = list(FloatReg)
        num_pickable_floatregs = prng.randint(
            MIN_NUM_PICKABLE_FLOATREGS, MAX_NUM_PICKABLE_FLOATREGS
        )
        allowed_floatregs = prng.sample(floatregs, num_pickable_floatregs)
        return allowed_floatregs

    def add_instr(self, new_instrobjs):
        """Adds instruction(s) to the hart instruction list"""
        if isinstance(new_instrobjs, list):
            self.basic_blocks[-1].extend(new_instrobjs)
        else:
            self.basic_blocks[-1].append(new_instrobjs)

    def add_interrupt_handler(self, interrupt_handler_block: list):
        self.interrupt_handlers.append(interrupt_handler_block)

    def save_reg_state(self):
        """stores the current states of registers"""
        self.saved_reg_states.append(self.intregpickstate.save_curr_state())

    def restore_previous_state(self):
        """
        Removes the current basic block for the generated program and
        restores registers to their states in the previous basic block
        """
        bb_to_remove_idx = len(self.basic_blocks) - 1
        for bb_idx, instr_idx in list(self.clint_instr):
            if bb_to_remove_idx == bb_idx:
                self.clint_instr.remove((bb_idx, instr_idx))
        self.basic_blocks.pop()
        self.bb_start_addrs.pop()
        self.intregpickstate.restore_state(self.saved_reg_states[-1])

    def has_reached_max_instr_num(
        self, nmax_instrs: Optional[int], new_fuzzing_instrs: int = 0
    ):
        """
        returns true if the current program has reached the maximal number
        of instructions
        """
        if nmax_instrs is None:
            return False
        else:
            total_fuzzing_instrs = self.total_fuzzing_instrs + new_fuzzing_instrs
            return total_fuzzing_instrs >= nmax_instrs

    def init_for_fuzzing(self, first_fuzzbb_base_addr: int):
        """
        Get the core ready for the fuzzing instruction generation
        """
        self.basic_blocks.append([])
        self.curr_bb_start_addr = first_fuzzbb_base_addr
        self.bb_start_addrs.append(self.get_curr_bb_start_addr())

    def init_new_bb(self):
        """
        initializes a new basic block
        Create a new basic block in the next hart, and update the start address
        of the current basic block in the next hart.
        Resets the next basic block address to none in the fuzzerstate until
        we choose a new one.
        Updates the list of basic blocks start address in the current basic block
        """
        self.basic_blocks.append([])
        self.curr_bb_start_addr = self.next_bb_addr
        self.next_bb_addr = None
        self.bb_start_addrs.append(self.get_curr_bb_start_addr())

    ##
    # Getter Methods
    ##

    def get_current_addr(self) -> int:
        """returns the address we are currently populating"""
        assert self.curr_bb_start_addr is not None
        return self.curr_bb_start_addr + ILEN * len(self.basic_blocks[-1])

    def get_producer_id_to_tgtaddr(self) -> dict:
        """Getter method for producer id to target address list"""
        return self.producer_id_to_tgtaddr

    def get_curr_bb_start_addr(self) -> int:
        """Getter method for the current basic block start address"""
        assert self.curr_bb_start_addr is not None
        return self.curr_bb_start_addr

    def get_next_bb_addr(self) -> int:
        """Getter method for the next bb address"""
        assert self.next_bb_addr is not None
        return self.next_bb_addr

    def get_bb_idx(self):
        """return the index of the current basic block"""
        return len(self.basic_blocks) - 1

    def get_cur_coord(self) -> tuple[int, int]:
        """returns the index of the next instruction coordinates in the
        instruction list
        """
        bb_id = self.get_bb_idx()
        instr_id = len(self.basic_blocks[-1])
        return (bb_id, instr_id)

    def is_memop_limit_reached(self):
        """returns true if we reached the target ammount of memops for this
        step
        """
        return self.step_memops_limit <= self.curr_n_memops
