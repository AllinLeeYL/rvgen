# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


"""
This module provides classes and methods for managing the state of integer and floating-point registers
during instruction generation and fuzzing. It includes functionality for picking input and output registers,
updating register states, and maintaining weights for register selection based on recent usage.

Classes:
    IntRegPickState: Manages the state and selection of integer registers.
    FloatRegPickState: Manages the state and selection of floating-point registers.
"""

from typing import Optional
from copy import copy, deepcopy
import math
from random import Random
from enum import IntEnum, auto
from params import REGPICK_PROTUBERANCE_RATIO
from riscv import IntReg, FloatReg

# TODO use generic to make a single classe instead of of for int and float
# TODO make a single function, which take the number of inputs and outputs as
# an argument


class RegType(IntEnum):
    INPUT = auto()
    OUTPUT = auto()


# ============================================================================
# Integer register handling
# ============================================================================
class IntRegState(IntEnum):
    FREE = auto()
    # Register used as offset, chnages between spike and RTL simulation
    PRODUCED0 = auto()
    PRODUCED1 = auto()
    CONSUMED = auto()
    # Register states for state preserving loops
    RESTRICTED = auto()
    INITIALIZED = auto()
    # Reserve reg for special use
    RESERVED = auto()
    # For regs that hold value which could mismatch between spike and rtl sim
    POLLUTED = auto()
    # regs that have a zero value
    ZEROED = auto()


# those are register whose value cannot change between pre-isa and rtl sim
INT_INPUT_STATES = [
    # normal execution
    IntRegState.FREE,
    IntRegState.ZEROED,
    # stateloop states
    IntRegState.RESTRICTED,
    IntRegState.INITIALIZED,
]

INT_OUTPUT_STATES = [
    # normal execution
    IntRegState.FREE,
    IntRegState.ZEROED,
    IntRegState.POLLUTED,
    # Stateloop states
    IntRegState.INITIALIZED,
]


class IntRegPickState:
    def __init__(self, pickable_intregs: list[IntReg], prng: Random):
        self.pickable_intregs = pickable_intregs
        self._reg_weights: dict[IntReg, float] = {
            reg: 1 / len(pickable_intregs) for reg in pickable_intregs
        }
        self._reg_states: dict[IntReg, IntRegState] = {
            reg: IntRegState.FREE for reg in pickable_intregs
        }
        self._regs_in_state: dict[IntRegState, list[IntReg]] = {
            state: deepcopy(pickable_intregs) if state == IntRegState.FREE else []
            for state in IntRegState
        }
        self._last_producer_ids: dict[IntReg, Optional[int]] = {
            reg: None for reg in pickable_intregs
        }
        self.prng: Random = prng
        self._in_stateloop = False
        self._recent_inputs: list[IntReg] = []

    ##
    # Private interface
    ##

    def _get_pickable_regs(self, reg_type: RegType) -> list[IntReg]:
        """Constructs the set of pickable regs, given the use of the register"""
        free_regs = []
        match reg_type:
            case RegType.INPUT:
                for reg_state in INT_INPUT_STATES:
                    free_regs += self._regs_in_state[reg_state]
                if self._in_stateloop:
                    self._ensure_remaning_outreg(free_regs)
            case RegType.OUTPUT:
                for reg_state in INT_OUTPUT_STATES:
                    free_regs += self._regs_in_state[reg_state]
                if self._in_stateloop:
                    free_regs = self._make_inputs_unpickable(free_regs)
            case _:
                raise ValueError
        return free_regs

    def _add_input(self, reg: IntReg):
        assert len(self._recent_inputs) < 2
        self._recent_inputs.append(reg)

    def _add_inputs(self, reg: list[IntReg]):
        assert len(self._recent_inputs) < 2
        self._recent_inputs.extend(reg)

    def _ensure_remaning_outreg(self, free_regs: list[IntReg]):
        """If there is only one output register remaining, exclude it from the
        input register list to preserve it for output use. When generating state
        preserving loops, the output registers set can deplete itself, and then
        no output registers can be picked.
        int instr can take at most 2 input, so reserve as soon as 2 are left
        """
        # Count the number of available output registers
        num_output_regs = self.get_num_int_outputs()
        # If only one output register remains, preserve it by excluding it from inputs
        if num_output_regs <= 2:
            output_regs = []
            for reg_state in INT_OUTPUT_STATES:
                output_regs += self.get_regs_in_state(reg_state)
            # Find which output reg(s) are in the free_regs list and remove one
            candidate_regs = [reg for reg in output_regs if reg in free_regs]
            if candidate_regs:
                reg_to_remove = self.prng.choice(candidate_regs)
                free_regs.remove(reg_to_remove)

    def _make_inputs_unpickable(self, free_regs: list[IntReg]):
        """Avoid using the register which have just been used as an input, as
        there states might not have been updated yet
        """
        updated_free_regs = []
        for reg in free_regs:
            if (
                reg not in self._recent_inputs
                or reg == IntReg.zero
                or self.get_regstate(reg) == IntRegState.INITIALIZED
            ):
                updated_free_regs.append(reg)
        return updated_free_regs

    def _update_probaweights(self, outreg: IntReg):
        """
        Updates the probability weights for register selection based on the most
        recently produced register.
        We favor more recently produced registers.

        Parameters:
        outreg (IntReg): The register whose weight should be updated.
        """
        if __debug__:
            assert outreg in self.pickable_intregs
            assert math.isclose(sum(self._reg_weights.values()), 1, abs_tol=0.001), (
                f"{sum(self._reg_weights.values())} {str(self._reg_weights)}"
            )

        sum_of_others = sum(self._reg_weights.values()) - self._reg_weights[outreg]
        weights_offset = (1 - REGPICK_PROTUBERANCE_RATIO) / sum_of_others
        for reg in self._reg_weights.keys():
            if reg == outreg:
                self._reg_weights[reg] = REGPICK_PROTUBERANCE_RATIO
            else:
                self._reg_weights[reg] *= weights_offset

    ##
    # Public interface
    ##

    def pick_random_integer_reg(self) -> IntReg:
        """Pick a random interger register in case the operation has no impact,
        like an exception
        """
        ret = self.prng.choice(self.pickable_intregs)
        return ret

    def pick_int_inputreg(self) -> IntReg:
        """ALWAYS PICK INPUTS BEFORE OUTPUTS !!!!

        Randomly selects an integer registers meant to be used as inputs from
        the set of pickable registers.

        Returns:
            IntReg: The selected integer register(s).
        """
        free_regs = self._get_pickable_regs(RegType.INPUT)
        weights = [self._reg_weights[reg] for reg in free_regs]
        ret = self.prng.choices(free_regs, weights=weights)[0]
        if self._in_stateloop:
            self._add_input(ret)
        return ret

    def pick_int_inputregs(self, n: int) -> list[IntReg]:
        """ALWAYS PICK INPUTS BEFORE OUTPUTS !!!!

        Randomly selects n integer registers meant to be used as inputs from
        the set of pickable registers.

        Parameters:
            n (int): The number of registers to select.

        Returns:
            list[IntReg]: The selected integer register(s).
        """
        assert n <= self.get_num_int_input()
        free_regs = self._get_pickable_regs(RegType.INPUT)
        weights = [self._reg_weights[reg] for reg in free_regs]
        ret = self.prng.choices(free_regs, weights=weights, k=n)
        if self._in_stateloop:
            self._add_inputs(ret)
        return ret

    def pick_int_inputreg_nonzero(self) -> IntReg:
        """
        ALWAYS PICK INPUTS BEFORE OUTPUTS !!!!

        Randomly selects an integer register meant to be used as an input from
        the set of pickable registers.
        Excludes the zero register.

        Returns:
            IntReg: The selected integer register.
        """
        free_regs = self._get_pickable_regs(RegType.INPUT)
        weights = [
            self._reg_weights[reg] if reg != IntReg.zero else 0 for reg in free_regs
        ]
        ret = self.prng.choices(free_regs, weights=weights)[0]
        if self._in_stateloop:
            self._add_input(ret)
        return ret

    def pick_int_outputreg(self, authorize_sideeffects: bool = True) -> IntReg:
        """
        ALWAYS PICK INPUTS BEFORE OUTPUTS !!!!

        Randomly selects an integer register meant to be used as an output from
        the set of pickable registers.
        Updates the register state if authorize_sideeffects is True.

        Parameters:
            authorize_sideeffects (bool): If True, update the selected register
            state.

        Returns:
            IntReg: The selected integer register.
        """
        free_regs = self._get_pickable_regs(RegType.OUTPUT)
        weights = [self._reg_weights[reg] for reg in free_regs]
        ret = self.prng.choices(free_regs, weights=weights)[0]
        if authorize_sideeffects:
            self._update_probaweights(ret)
        if self.get_regstate(ret, allow_nonpickable=True) in (
            IntRegState.POLLUTED,
            IntRegState.ZEROED,
        ):
            self.set_regstate(ret, IntRegState.FREE)
        return ret

    def pick_int_outputreg_nonzero(self, authorize_sideeffects: bool = True) -> IntReg:
        """
        ALWAYS PICK INPUTS BEFORE OUTPUTS !!!!

        Randomly selects an integer register meant to be used as an output from
        the set of pickable registers.
        Excludes the zero register. Updates the register state if
        authorize_sideeffects is True.

        Parameters:
            authorize_sideeffects (bool): If True, update the selected register
            state.

        Returns:
            IntReg: The selected integer register.
        """
        free_regs = self._get_pickable_regs(RegType.OUTPUT)
        weights = [
            self._reg_weights[reg] if reg != IntReg.zero else 0 for reg in free_regs
        ]
        ret = self.prng.choices(free_regs, weights=weights)[0]
        if authorize_sideeffects:
            self._update_probaweights(ret)
        if self.get_regstate(ret, allow_nonpickable=True) in (
            IntRegState.POLLUTED,
            IntRegState.ZEROED,
        ):
            self.set_regstate(ret, IntRegState.FREE)
        return ret

    def pick_int_outputregs_nonzero(
        self, n: int, authorize_sideeffects: bool = True
    ) -> list[IntReg]:
        """
        ALWAYS PICK INPUTS BEFORE OUTPUTS !!!!

        Randomly selects an integer register meant to be used as an output from
        the set of pickable registers.
        Excludes the zero register. Updates the register state if
        authorize_sideeffects is True.

        Parameters:
            authorize_sideeffects (bool): If True, update the selected register
            state.

        Returns:
            IntReg: The selected integer register.
        """
        assert n <= self.get_num_int_outputs()
        free_regs = self._get_pickable_regs(RegType.OUTPUT)
        weights = [
            self._reg_weights[reg] if reg != IntReg.zero else 0 for reg in free_regs
        ]
        ret = None
        while ret is None or len(ret) != len(set(ret)):
            ret = self.prng.choices(free_regs, weights=weights, k=n)
        for reg in ret:
            if authorize_sideeffects:
                self._update_probaweights(reg)
            if self.get_regstate(reg, allow_nonpickable=True) in (
                IntRegState.POLLUTED,
                IntRegState.ZEROED,
            ):
                self.set_regstate(reg, IntRegState.FREE)
        return ret

    def pick_reg_in_state(self, req_state: IntRegState) -> IntReg:
        """
        Randomly selects an integer register in the requested state.

        Parameters:
            req_state (IntRegState): The requested state of the register.

        Returns:
            IntReg: The selected integer register.
        """
        regs_in_state = self._regs_in_state[req_state]
        weights = [self._reg_weights[reg] for reg in regs_in_state]
        ret = self.prng.choices(regs_in_state, weights=weights)[0]
        return ret

    ##
    # Do not allow using an input register as an output register
    ##

    def enter_stateloop(self):
        """Ensures that some regs stay in the FREE state and do not get
        restricted by removing free regs fronm the pickable reg list if there
        are too little FREE regs to ensure at least one stays free.
        Used during stateloop generation
        """
        if __debug__:
            assert self.get_num_regs_in_state(IntRegState.INITIALIZED) == 0
            assert self.get_num_regs_in_state(IntRegState.RESTRICTED) == 0
            assert self._in_stateloop == False
        self._in_stateloop = True

    def exit_stateloop(self):
        """Allow all regs to be picked again"""
        if __debug__:
            assert self._in_stateloop == True
        self._in_stateloop = False
        restricted_regs = deepcopy(self.get_regs_in_state(IntRegState.RESTRICTED))
        for reg in restricted_regs:
            self.set_regstate(reg, IntRegState.FREE)
        initialized_regs = deepcopy(self.get_regs_in_state(IntRegState.INITIALIZED))
        for reg in initialized_regs:
            self.set_regstate(reg, IntRegState.FREE)
        if __debug__:
            assert self.get_num_regs_in_state(IntRegState.INITIALIZED) == 0
            assert self.get_num_regs_in_state(IntRegState.RESTRICTED) == 0

    def reset_recent_inputs(self):
        """Resets the list of recent inputs. Used to ensure the inputs
        of the current instruction are not chosen as outputs in a stateloop
        """
        self._recent_inputs = []

    ##
    # Getter and setter for register states
    ##

    def get_regstate(self, reg: IntReg, allow_nonpickable: bool = False) -> IntRegState:
        """
        Gets the state of the specified register.

        Parameters:
            reg_id (int): The ID of the register.

        Returns:
            IntRegState: The state of the register.
        """
        if reg not in self.pickable_intregs and allow_nonpickable:
            return IntRegState.FREE
        if reg == IntReg.zero:
            return IntRegState.FREE
        assert reg in self.pickable_intregs and reg != IntReg.zero
        return self._reg_states[reg]

    def set_regstate(self, reg: IntReg, new_state: IntRegState, force: bool = False):
        """
        Sets the state of the specified register.

        Parameters:
            reg (IntReg): The register to set the state for.
            new_state (int): The new state of the register.
            force (bool): If True, do not check compatibility before->after.
            Used for restoring some saved state.
        """
        if reg == IntReg.zero:
            return
        if __debug__:
            assert reg in self.pickable_intregs, f"{reg}"
            if not force:
                if self._reg_states[reg] == IntRegState.FREE:
                    assert new_state in (
                        IntRegState.PRODUCED0,
                        IntRegState.RESTRICTED,
                        IntRegState.INITIALIZED,
                        IntRegState.RESERVED,
                        IntRegState.POLLUTED,
                    )
                elif self._reg_states[reg] == IntRegState.POLLUTED:
                    assert new_state in (
                        IntRegState.FREE,
                        IntRegState.ZEROED,
                        IntRegState.RESTRICTED,
                        IntRegState.INITIALIZED,
                    )
                elif self._reg_states[reg] == IntRegState.PRODUCED0:
                    assert new_state == IntRegState.PRODUCED1
                elif self._reg_states[reg] == IntRegState.PRODUCED1:
                    assert new_state == IntRegState.CONSUMED
                elif self._reg_states[reg] in (
                    IntRegState.CONSUMED,
                    IntRegState.RESTRICTED,
                    IntRegState.INITIALIZED,
                    IntRegState.RESERVED,
                    IntRegState.POLLUTED,
                ):
                    assert new_state == IntRegState.FREE, (
                        self._reg_states[reg],
                        new_state,
                    )
        assert reg != IntReg.zero
        self._regs_in_state[self._reg_states[reg]].remove(reg)
        self._regs_in_state[new_state].append(reg)
        self._reg_states[reg] = new_state

    ##
    # Getters and setters for producer ids
    ##

    def get_producer_id(self, reg: IntReg) -> int:
        """
        Gets the producer ID of the specified register.

        Parameters:
            reg (IntReg): The register to get the producer ID for.

        Returns:
            int: The producer ID of the register.
        """
        producer_id = self._last_producer_ids[reg]
        assert producer_id
        return producer_id

    def set_producer_id(self, reg: IntReg, producer_id: int):
        """
        Sets the producer ID of the specified register.

        Parameters:
            reg (IntReg): The register to set the producer ID for.
            producer_id (int): The producer ID to set.
        """
        self._last_producer_ids[reg] = producer_id

    def get_regs_in_state(self, req_state: IntRegState) -> list[IntReg]:
        """
        Gets the registers currently in the requested state.

        Parameters:
            req_state (IntRegState): The requested state of the registers.

        Returns:
            int: The number of registers in the requested state.
        """
        return list(self._regs_in_state[req_state])

    def exists_reg_in_state(self, req_state: IntRegState) -> bool:
        """
        Checks if there exists a register in the requested state.

        Parameters:
            req_state (IntRegState): The requested state of the register.

        Returns:
            bool: True if there exists a register in the requested state, False otherwise.
        """
        return bool(self._regs_in_state[req_state])

    def get_num_regs_in_state(self, req_state: IntRegState) -> int:
        """
        Gets the number of registers in the requested state.

        Parameters:
            req_state (IntRegState): The requested state of the registers.

        Returns:
            int: The number of registers in the requested state.
        """
        return len(self._regs_in_state[req_state])

    def get_num_int_input(self) -> int:
        """
        Returns the current number of input registers

        Returns:
            int: the current number of input registers
        """
        n_input_regs = 0
        for reg_state in INT_INPUT_STATES:
            n_input_regs += self.get_num_regs_in_state(reg_state)
        return n_input_regs

    def get_num_int_outputs(self) -> int:
        """
        Returns the current number of output registers

        Returns:
            int: the current number of output registers
        """
        n_output_regs = 0
        for reg_state in INT_OUTPUT_STATES:
            n_output_regs += self.get_num_regs_in_state(reg_state)
        return n_output_regs

    def save_curr_state(self):
        """
        Saves the current state of the registers.

        Returns:
            tuple: The saved state of the registers.
        """
        return (
            copy(self._reg_weights),
            copy(self._reg_states),
            copy(self._last_producer_ids),
        )

    def restore_state(self, saved_state: tuple):
        """
        Restores the state of the registers from the saved state. Rarely called

        Parameters:
            saved_state (tuple): The saved state of the registers.
        """
        if __debug__:
            assert len(saved_state) == 3
        self._reg_weights = copy(saved_state[0])
        # Restore the reg states. Be careful to also restore the internal matrix.
        # Therefore, use the API function.
        for reg in self.pickable_intregs:
            if reg == IntReg.zero:
                continue
            self.set_regstate(reg, saved_state[1][reg], force=True)
        self._last_producer_ids = copy(saved_state[2])


# ============================================================================
# Integer register handling
# ============================================================================


class FloatRegState(IntEnum):
    FREE = auto()
    # Register states for state preserving loops
    RESTRICTED = auto()
    INITIALIZED = auto()
    # For regs that hold value which could mismatch between spike and rtl sim
    POLLUTED = auto()


FLOAT_INPUT_STATES = [
    # normal execution
    FloatRegState.FREE,
    # stateloop states
    FloatRegState.RESTRICTED,
    FloatRegState.INITIALIZED,
]

FLOAT_OUTPUT_STATES = [
    # normal execution
    FloatRegState.FREE,
    FloatRegState.POLLUTED,
    # Stateloop states
    FloatRegState.INITIALIZED,
]


class FloatRegPickState:
    def __init__(self, pickable_floatregs: list[FloatReg], prng: Random):
        self.pickable_floatregs = pickable_floatregs
        self._reg_weights: dict[FloatReg, float] = {
            reg: 1 / len(pickable_floatregs) for reg in pickable_floatregs
        }
        self._reg_states: dict[FloatReg, FloatRegState] = {
            reg: FloatRegState.FREE for reg in pickable_floatregs
        }
        self._regs_in_state: dict[FloatRegState, list[FloatReg]] = {
            state: pickable_floatregs.copy() if state == FloatRegState.FREE else []
            for state in FloatRegState
        }
        self.prng: Random = prng
        self._recent_inputs: list[FloatReg] = []
        self._in_stateloop = False

    def _get_pickable_regs(self, reg_type: RegType) -> list[FloatReg]:
        """Constructs the set of pickable regs, given the use of the register"""
        free_regs = []
        match reg_type:
            case RegType.INPUT:
                for reg_state in FLOAT_INPUT_STATES:
                    free_regs += self._regs_in_state[reg_state]
                if self._in_stateloop:
                    self._ensure_remaning_outreg(free_regs)
            case RegType.OUTPUT:
                for reg_state in FLOAT_OUTPUT_STATES:
                    free_regs += self._regs_in_state[reg_state]
                if self._in_stateloop:
                    free_regs = self._make_inputs_unpickable(free_regs)
            case _:
                raise ValueError
        return free_regs

    def exists_reg_in_state(self, req_state: FloatRegState) -> bool:
        """
        Checks if there exists a register in the requested state.

        Parameters:
            req_state (FloatRegState): The requested state of the register.

        Returns:
            bool: True if there exists a register in the requested state, False otherwise.
        """
        return bool(self._regs_in_state[req_state])

    def pick_reg_in_state(self, req_state: FloatRegState) -> FloatReg:
        """
        Randomly selects an integer register in the requested state.

        Parameters:
            req_state (FloatRegState): The requested state of the register.

        Returns:
            FloatReg: The selected integer register.
        """
        regs_in_state = self._regs_in_state[req_state]
        weights = [self._reg_weights[reg] for reg in regs_in_state]
        ret = self.prng.choices(regs_in_state, weights=weights)[0]
        return ret

    def enter_stateloop(self):
        """Ensures that some regs stay in the FREE state and do not get
        restricted by removing free regs fronm the pickable reg list if there
        are too little FREE regs to ensure at least one stays free
        """
        if __debug__:
            assert self.get_num_regs_in_state(FloatRegState.INITIALIZED) == 0
            assert self.get_num_regs_in_state(FloatRegState.RESTRICTED) == 0
            assert self._in_stateloop == False
        self._in_stateloop = True

    def exit_stateloop(self):
        """Allow all regs to be picked again"""
        assert self._in_stateloop == True
        self._in_stateloop = False
        regs = deepcopy(self.get_regs_in_state(FloatRegState.RESTRICTED))
        for reg in regs:
            self.set_regstate(reg, FloatRegState.FREE)
        regs = deepcopy(self.get_regs_in_state(FloatRegState.INITIALIZED))
        for reg in regs:
            self.set_regstate(reg, FloatRegState.FREE)
        if __debug__:
            assert self.get_num_regs_in_state(FloatRegState.INITIALIZED) == 0
            assert self.get_num_regs_in_state(FloatRegState.RESTRICTED) == 0
            assert self._in_stateloop == False

    def reset_recent_inputs(self):
        """Clears the recent inputs for stateloop generation"""
        self._recent_inputs = []

    def pick_random_float_reg(self) -> FloatReg:
        """Pick a random fp reg"""
        fp_reg = self.prng.choice(self.pickable_floatregs)
        return fp_reg

    def pick_float_inputreg(self) -> FloatReg:
        """
        Pick a floating point register meant to be used as an input to an instruction.
        Consuming a register does not update the float pick state.

        Returns:
            FloatReg: The selected floating point reg.
        """
        free_regs = self._get_pickable_regs(RegType.INPUT)
        weights = [self._reg_weights[reg] for reg in free_regs]
        ret = self.prng.choices(free_regs, weights=weights)[0]
        if self._in_stateloop:
            self._add_input(ret)
        return ret

    def pick_float_inputregs(self, n: int) -> list[FloatReg]:
        """
        Pick a floating point register meant to be used as an input to an instruction.
        Consuming a register does not update the float pick state.

        Returns:
            list[FloatReg]: The selected floating point regs.
        """
        free_regs = self._get_pickable_regs(RegType.INPUT)
        weights = [self._reg_weights[reg] for reg in free_regs]
        assert free_regs != [], (self._reg_states, self.pickable_floatregs)
        ret = self.prng.choices(free_regs, weights=weights, k=n)
        if self._in_stateloop:
            self._add_inputs(ret)
        return ret

    def pick_float_outputreg(self):
        """Pick a floating point register meant to be used as an output to an instruction.
        Consuming a register does updates the float pick state.

        Returns:
            FloatReg: The selected floating point reg.
        """
        free_regs = self._get_pickable_regs(RegType.OUTPUT)
        weights = [self._reg_weights[reg] for reg in free_regs]
        ret = self.prng.choices(free_regs, weights=weights)[0]
        self._update_floatregstate(ret)
        if self.get_regstate(ret, allow_nonpickable=True) == FloatRegState.POLLUTED:
            self.set_regstate(ret, FloatRegState.FREE)
        return ret

    def set_regstate(self, reg: FloatReg, new_state: FloatRegState):
        """
        Sets the state of the specified register.

        Parameters:
            reg (IntReg): The register to set the state for.
            new_state (int): The new state of the register.
            Used for restoring some saved state.
        """
        if __debug__:
            assert reg in self.pickable_floatregs, f"{reg}"
            if self._reg_states[reg] == FloatRegState.FREE:
                assert new_state in (
                    FloatRegState.RESTRICTED,
                    FloatRegState.INITIALIZED,
                    FloatRegState.POLLUTED,
                )
            elif self._reg_states[reg] in (
                FloatRegState.RESTRICTED,
                FloatRegState.INITIALIZED,
                FloatRegState.POLLUTED,
            ):
                assert new_state == FloatRegState.FREE, new_state

        prev_state = self._reg_states[reg]
        self._regs_in_state[prev_state].remove(reg)
        self._regs_in_state[new_state].append(reg)
        self._reg_states[reg] = new_state

    def get_pickable_regs(self) -> list[FloatReg]:
        """returns the set of pickable floatregs"""
        return list(self.pickable_floatregs)

    def get_regs_in_state(self, req_state: FloatRegState) -> list[FloatReg]:
        """
        Gets the registers currently in the requested state.

        Parameters:
            req_state (IntRegState): The requested state of the registers.

        Returns:
            int: The number of registers in the requested state.
        """
        return list(self._regs_in_state[req_state])

    def get_regstate(
        self, reg: FloatReg, allow_nonpickable: bool = False
    ) -> FloatRegState:
        """
        Gets the state of the specified register.

        Parameters:
            reg_id (int): The ID of the register.

        Returns:
            FloatRegState: The state of the register.
        """
        if reg not in self.pickable_floatregs and allow_nonpickable:
            return FloatRegState.FREE
        return self._reg_states[reg]

    def get_num_regs_in_state(self, req_state: FloatRegState) -> int:
        """
        Gets the number of registers in the requested state.

        Parameters:
            req_state (FloatRegState): The requested state of the registers.

        Returns:
            int: The number of registers in the requested state.
        """
        return len(self._regs_in_state[req_state])

    def get_num_float_input(self) -> int:
        """
        Returns the current number of input registers

        Returns:
            int: the current number of input registers
        """
        n_input_regs = 0
        for reg_state in FLOAT_INPUT_STATES:
            n_input_regs += self.get_num_regs_in_state(reg_state)
        return n_input_regs

    def get_num_float_outputs(self) -> int:
        """
        Returns the current number of output registers

        Returns:
            int: the current number of output registers
        """
        n_output_regs = 0
        for reg_state in FLOAT_OUTPUT_STATES:
            n_output_regs += self.get_num_regs_in_state(reg_state)
        return n_output_regs

    def _update_floatregstate(self, outreg: FloatReg):
        """Updates the probability weights for register selection based on the most
        recently produced register.
        We favor more recently produced registers.

        Parameters:
            outreg (IntReg): The register whose weight should be updated.
        """
        if __debug__:
            assert outreg in self.pickable_floatregs
            assert math.isclose(sum(self._reg_weights.values()), 1, abs_tol=0.001), (
                f"{sum(self._reg_weights.values())} {str(self._reg_weights)}"
            )
        # If there is a single one, we do not want to zero its weight
        if len(self.pickable_floatregs) > 1:
            sum_of_others = sum(self._reg_weights.values()) - self._reg_weights[outreg]
            weights_offset = (1 - REGPICK_PROTUBERANCE_RATIO) / sum_of_others
            for freg in self.pickable_floatregs:
                if freg == outreg:
                    self._reg_weights[freg] = REGPICK_PROTUBERANCE_RATIO
                else:
                    self._reg_weights[freg] *= weights_offset

    def _add_input(self, reg: FloatReg):
        assert len(self._recent_inputs) < 3
        self._recent_inputs.append(reg)

    def _add_inputs(self, reg: list[FloatReg]):
        assert len(self._recent_inputs) < 3
        self._recent_inputs.extend(reg)

    def _ensure_remaning_outreg(self, free_regs: list[FloatReg]):
        """Ensure there are enough output registers
        Since fp instruction can take up to 3 registers as input, reserve one as
        soon as the is less than 3 registers
        """
        # Count the number of available output registers
        num_output_regs = self.get_num_float_outputs()
        if num_output_regs <= 3:
            output_regs = []
            for reg_state in FLOAT_OUTPUT_STATES:
                output_regs += self.get_regs_in_state(reg_state)
            # Find which output reg(s) are in the free_regs list and remove one
            candidate_regs = [reg for reg in output_regs if reg in free_regs]
            if candidate_regs:
                reg_to_remove = self.prng.choice(candidate_regs)
                free_regs.remove(reg_to_remove)

    def _make_inputs_unpickable(self, free_regs):
        updated_free_regs = [reg for reg in free_regs if reg not in self._recent_inputs]
        return updated_free_regs
