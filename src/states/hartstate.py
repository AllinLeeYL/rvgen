# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only
"""
This class keeps track of the state of the actual hardware. The main purpose of
tracking the harware state is to know the state of CSR registers during
generation to know if an exception or other operation dependant on the state can
be produced
"""


from riscv import ExceptionCause, PrivLvl, FpuState
from functools import reduce
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from params import TestParams


class HartState:
    """This class is used to keep track of the states of some registers, which
    are necessary to generate valid programs. All relevant states are
    initialized in the initial basic block to avoid random initial values
    """

    def __init__(self, design_has_fpu: bool):
        # All the register that track supervisor mode states can be accessed
        # from supervior mode, using the corresponding S-mode CSR. They are
        # identical to the M-mode states (e.g. sstatus is a restricted view of
        # mstatus)
        self.privlvl: PrivLvl = PrivLvl.Machine

        self.mstatus_mpp: PrivLvl = PrivLvl.Machine
        self.mstatus_spp: PrivLvl = PrivLvl.Supervisor
        self.mstatus_mpie: bool = False
        self.mstatus_spie: bool = False
        self.mstatus_mie: bool = True
        self.mstatus_sie: bool = True
        self.mstatus_fs: FpuState = FpuState.Dirty if design_has_fpu else FpuState.Off
        self.mstatus_mdt: bool = False

        self.mie_msie: bool = True
        self.mie_ssie: bool = True

        self.mip_msip: bool = False
        self.mip_ssip: bool = False

        self.is_mtvec_populated: bool = False
        self.is_stvec_populated: bool = False
        self.is_mepc_populated: bool = False
        self.is_sepc_populated: bool = False

        self.medeleg: int = 0
        self.mideleg: int = 0

        # We must ignore the reset value of some CSRs because they are randomly
        # initialized during the RTL simulation
        self.is_mtvec_non_deterministic: bool = True
        self.is_stvec_still_reset_val: bool = True
        self.is_mepc_non_deterministic: bool = True
        self.is_sepc_still_reset_val: bool = True

    def set_mstatus_fs(self, new_state: FpuState):
        """sets the fpu states"""
        self.mstatus_fs = new_state

    def is_ready_to_descend_privileges(
        self, test_params: "TestParams", has_consumed_reg: bool
    ) -> bool:
        """
        Returns true if the hart can issue a priviledge descent instruction.
        Descending priviledge modes is only supported on designs with all
        modes available (MSU).

        If the current state is machine, then we can descend privileges only if we
        can come back.
        - If we go to supervisor, only if mtvec is required.
        - If we go to user, we require mtvec, and stvec if all exceptions are
        delegated.

        However, this ISA class still encompasses setting mpp and spp bits, to we
        tolerate this ISA class at all times when executing as a non-user.
        """

        if not test_params.authorize_privileges:
            return False
        if not test_params.s_mode_support or not test_params.u_mode_support:
            return False
        if self.is_mepc_non_deterministic:
            return False

        # Check if the mret instruction would trigger a trap, and if we can
        # handle it if a trap happens
        if (
            self.privlvl == PrivLvl.Machine
            and self.mip_msip
            and self.mie_msie
            and (
                self.mstatus_mpp != PrivLvl.Machine
                or (self.mstatus_mpp == PrivLvl.Machine and self.mstatus_mpie)
            )
            and self.is_mtvec_populated
        ):
            return True

        takable_exception_mask = self.gen_takable_exception_mask(
            test_params, has_consumed_reg=False
        )
        can_go_back = self.is_mtvec_populated and (
            self.is_stvec_populated or bool((~self.medeleg) & takable_exception_mask)
        )

        # To quit U-mode, we have to trap
        match self.privlvl:
            case PrivLvl.User:
                return False
            case PrivLvl.Machine:
                if not self.is_mepc_populated:
                    return False
                elif self.mstatus_mpp == PrivLvl.User:
                    return can_go_back
                elif self.mstatus_mpp in (PrivLvl.Supervisor, PrivLvl.Machine):
                    return self.is_mtvec_populated
                else:
                    raise ValueError("state unknown, cannot proceed")
            case PrivLvl.Supervisor:
                if not self.is_sepc_populated:
                    return False
                else:
                    return can_go_back
            case _:
                raise NotImplementedError(
                    "Unknown privilege state: " + str(self.privlvl)
                )

        # The below for implementing sret in machine mode
        # elif fuzzerstate.hartstate.privlvl == PrivLvl.Machine:
        #     if not fuzzerstate.hartstate.is_mepc_populated \
        #       and (not fuzzerstate.s_mode_support \
        #       or not fuzzerstate.hartstate.is_sepc_populated):
        #         return False
        #     if fuzzerstate.hartstate.mstatus_mpp == PrivLvl.Machine \
        #         or fuzzerstate.hartstate.mstatus_mpp == PrivLvl.Supervisor:
        #         return fuzzerstate.hartstate.is_mtvec_populated

        # mpp == Machine or Supervisor case

    def gen_takable_exception_dict(
        self, test_params: "TestParams", has_consumed_reg: bool
    ):
        takable_exceptions = {
            ExceptionCause.ID_INSTR_ADDR_MISALIGNED: True,
            ExceptionCause.ID_ILLEGAL_INSTRUCTION: True,
            ExceptionCause.ID_BREAKPOINT: True,
            ExceptionCause.ID_LOAD_ADDR_MISALIGNED: True,
            ExceptionCause.ID_STORE_AMO_ADDR_MISALIGNED: True,
            ExceptionCause.ID_ENVIRONMENT_CALL_FROM_U_MODE: True,
            ExceptionCause.ID_ENVIRONMENT_CALL_FROM_S_MODE: True,
            ExceptionCause.ID_ENVIRONMENT_CALL_FROM_M_MODE: True,
            ExceptionCause.ID_INSTR_ACCESS_FAULT: False,
            ExceptionCause.ID_LOAD_ACCESS_FAULT: False,
            ExceptionCause.ID_STORE_AMO_ACCESS_FAULT: False,
            # Not yet supported
            ExceptionCause.ID_INSTRUCTION_PAGE_FAULT: False,
            ExceptionCause.ID_LOAD_PAGE_FAULT: False,
            ExceptionCause.ID_STORE_AMO_PAGE_FAULT: False,
        }

        # Remove all entries with weight zero
        for exception_cause_val in list(takable_exceptions.keys()):
            if test_params.exceptionoppickweights[exception_cause_val] == 0:
                takable_exceptions[exception_cause_val] = False

        if test_params.misaligned_data_support:
            # If such data accesses are supported, then it will not result in an
            # exception
            takable_exceptions[ExceptionCause.ID_LOAD_ADDR_MISALIGNED] = False
            takable_exceptions[ExceptionCause.ID_STORE_AMO_ADDR_MISALIGNED] = False

        if test_params.c_ext_support:
            takable_exceptions[ExceptionCause.ID_INSTR_ADDR_MISALIGNED] = False

        # Not yet sure whether we will need a consumed reg, but let's do it
        # like this for now
        if not has_consumed_reg:
            takable_exceptions[ExceptionCause.ID_LOAD_ACCESS_FAULT] = False
            takable_exceptions[ExceptionCause.ID_STORE_AMO_ACCESS_FAULT] = False
            takable_exceptions[ExceptionCause.ID_INSTRUCTION_PAGE_FAULT] = False
            takable_exceptions[ExceptionCause.ID_LOAD_PAGE_FAULT] = False
            takable_exceptions[ExceptionCause.ID_STORE_AMO_PAGE_FAULT] = False
            takable_exceptions[ExceptionCause.ID_LOAD_ADDR_MISALIGNED] = False
            takable_exceptions[ExceptionCause.ID_STORE_AMO_ADDR_MISALIGNED] = False

        if self.privlvl == PrivLvl.Machine:
            takable_exceptions[ExceptionCause.ID_ENVIRONMENT_CALL_FROM_S_MODE] = False
            takable_exceptions[ExceptionCause.ID_ENVIRONMENT_CALL_FROM_U_MODE] = False

            if not self.is_mtvec_populated:
                # Forbid all exceptions
                takable_exceptions[ExceptionCause.ID_INSTR_ADDR_MISALIGNED] = False
                takable_exceptions[ExceptionCause.ID_INSTR_ACCESS_FAULT] = False
                takable_exceptions[ExceptionCause.ID_ILLEGAL_INSTRUCTION] = False
                takable_exceptions[ExceptionCause.ID_BREAKPOINT] = False
                takable_exceptions[ExceptionCause.ID_LOAD_ADDR_MISALIGNED] = False
                takable_exceptions[ExceptionCause.ID_STORE_AMO_ADDR_MISALIGNED] = False
                takable_exceptions[ExceptionCause.ID_ENVIRONMENT_CALL_FROM_M_MODE] = (
                    False
                )

            return takable_exceptions
        else:
            # FIXME no need for the nested if statement
            if self.privlvl == PrivLvl.Supervisor:
                takable_exceptions[ExceptionCause.ID_ENVIRONMENT_CALL_FROM_M_MODE] = (
                    False
                )
                takable_exceptions[ExceptionCause.ID_ENVIRONMENT_CALL_FROM_U_MODE] = (
                    False
                )
            elif self.privlvl == PrivLvl.User:
                takable_exceptions[ExceptionCause.ID_ENVIRONMENT_CALL_FROM_M_MODE] = (
                    False
                )
                takable_exceptions[ExceptionCause.ID_ENVIRONMENT_CALL_FROM_S_MODE] = (
                    False
                )

            # Check whether the corresponding mtvec/stvec is populated
            # (depending on current privilege state and delegation)
            for exception_cause_val in list(takable_exceptions.keys()):
                if self.privlvl != PrivLvl.Machine:
                    if self.medeleg & (1 << exception_cause_val.value):
                        # Delegated to supervisor mode
                        if not self.is_stvec_populated:
                            takable_exceptions[exception_cause_val] = False
                    else:
                        if not self.is_mtvec_populated:
                            takable_exceptions[exception_cause_val] = False

            return takable_exceptions

    def gen_takable_exception_mask(
        self, test_params: "TestParams", has_consumed_reg: bool
    ) -> int:
        """Generates a mask of the exception that can be taken"""
        ret = 0
        for k, v in self.gen_takable_exception_dict(
            test_params, has_consumed_reg
        ).items():
            ret |= v << k
        return ret

    # FUTURE Add page faults and access faults here.
    def is_ready_to_take_exception(
        self, test_params: "TestParams", has_consumed_reg: bool
    ):
        curr_dict = self.gen_takable_exception_dict(test_params, has_consumed_reg)
        return reduce(lambda x, y: x | y, list(curr_dict.values()))
