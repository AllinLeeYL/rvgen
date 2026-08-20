# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


# This module is responsible for picking specific operations among exceptions.

from proggen.util import gen_random_rounding_mode
from instgen import (
    JALInstr,
    SimpleIllegalInstr,
    TrapInstrWrapper,
    MisalignedMemInstr,
    EcallEbreakInstr,
    CSRRegInstr,
    PrivDescentInstr,
    Float3Instr,
    Float3Instrs,
)
from states import IntRegState
from riscv import (
    IntReg,
    FloatReg,
    ExceptionCause,
    PrivLvl,
    FpuState,
    ExceptionCause,
    CSR,
    INTERESTING_CSRS_INACCESSIBLE_FROM_SUPERVISOR,
    INTERESTING_CSRS_INACCESSIBLE_FROM_USER,
)
from toleratebugs import (
    is_tolerate_rocket_minstret,
    is_tolerate_kronos_readbadcsr,
    is_tolerate_picorv32_readnonimplcsr,
    is_forbid_vexriscv_csrs,
    is_tolerate_vexriscv_fpu_disabled,
    is_tolerate_vexriscv_fpu_leak,
)
from params import (
    SIMPLE_ILLEGAL_INSTRUCTION_PROBA,
    PROBA_PICK_WRONG_FPU,
)
from copy import copy
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from states import FuzzerState, CoreState
    from params import TestParams

###
# Exception type
###


# @param weights a list either None (equal weights) or as long as ExceptionCause
# return an ExceptionCause
# Do NOT @cache this function, as it is a random function.
def _gen_next_exceptionoptype(
    corestate: "CoreState",
    test_params: "TestParams",
    exceptionweights: dict[ExceptionCause, float],
    has_consumed_reg: bool,
) -> ExceptionCause:
    # Depending on delegations and whether mtvec and stvec are populated,
    # select which exceptions are possible
    weights_dict = copy(
        _get_exceptionoptype_filtered_weights(
            corestate, test_params, exceptionweights, has_consumed_reg
        )
    )
    if __debug__:
        assert sum(weights_dict.values()) > 0
    # If there's a single possibility, then return that one
    for curr_exception_type, curr_weight in weights_dict.items():
        if curr_weight == 1:
            return curr_exception_type
    ret = None
    weights = list(weights_dict.values())
    ret = test_params.prng.choices(list(ExceptionCause), weights=weights)[0]
    assert weights != None
    return ret


# @brief For now, the weights used for choosing instructions are fixed over time.
# This function filters the exceptionoptype weights according to the capabilities of a given CPU
# @return a normalized dict of ExceptionCauseVals that can currently be used.
# DO NOT @cache
def _get_exceptionoptype_filtered_weights(
    corestate: "CoreState",
    test_params: "TestParams",
    exceptionweights: dict[ExceptionCause, float],
    has_consumed_reg: bool,
) -> dict[ExceptionCause, float]:
    takable_exceptions = corestate.hartstate.gen_takable_exception_dict(
        test_params, has_consumed_reg
    )
    ret_dict = {
        exception_type: int(takable_exceptions[exception_type])
        * exceptionweights[exception_type]
        for exception_type in exceptionweights
    }
    # Normalize the weights
    if __debug__:
        assert sum(ret_dict.values()) > 0, (
            "The sum of filtered exceptionop pick weights must be strictly positive! Currently: "
            + str(sum(ret_dict.values()))
        )
    norm_factor = 1 / sum(ret_dict.values())
    for curr_key in ret_dict:
        ret_dict[curr_key] = ret_dict[curr_key] * norm_factor
    return ret_dict


###
# from ..exception type, pick an exception instruction
###


# Illegal instructions can result from ..either a simple non-existing instruction,
# or from ..an illegal CSR access.
# Warning: the privilege state of fuzzerstate is already updated!!
# @param old_privilege the privilege state before the exception
def _pick_illegal_instruction(
    is_mtvec: bool,
    corestate: "CoreState",
    test_params: "TestParams",
    old_privilege: PrivLvl,
):
    is_fpu_activated = corestate.hartstate.mstatus_fs != FpuState.Off
    if (
        "vexriscv" in test_params.design_name
        and is_tolerate_vexriscv_fpu_disabled()
        and not is_fpu_activated
    ):
        return TrapInstrWrapper(
            is_mtvec,
            None,
            Float3Instr("fadd.s", FloatReg.ft0, FloatReg.ft0, FloatReg.ft0, 0, False),
            old_privilege,
        )
    if (
        "vexriscv" in test_params.design_name
        and is_tolerate_vexriscv_fpu_leak()
        and is_fpu_activated
    ):
        rs1 = corestate.intregpickstate.pick_random_integer_reg()
        rd = corestate.intregpickstate.pick_random_integer_reg()
        return TrapInstrWrapper(
            is_mtvec,
            None,
            CSRRegInstr("csrrw", rd, rs1, CSR.FCSR),
            old_privilege,
        )

    if "vexriscv" in test_params.design_name and is_forbid_vexriscv_csrs():
        corrected_simple_illegal_instruction_proba = 1
    else:
        corrected_simple_illegal_instruction_proba = SIMPLE_ILLEGAL_INSTRUCTION_PROBA
    if corrected_simple_illegal_instruction_proba < test_params.prng.random():
        return SimpleIllegalInstr(is_mtvec, old_privilege)

    if (
        not test_params.design_has_fpu
        or not is_fpu_activated
        and not ("vexriscv" in test_params.design_name and is_forbid_vexriscv_csrs())
    ):
        if (
            test_params.prng.random()
            < PROBA_PICK_WRONG_FPU * 0.01**test_params.design_has_fpu
        ):
            rm = gen_random_rounding_mode(test_params.prng)
            frs1 = corestate.floatregpickstate.pick_random_float_reg()
            frs2 = corestate.floatregpickstate.pick_random_float_reg()
            frd = corestate.floatregpickstate.pick_random_float_reg()
            return TrapInstrWrapper(
                is_mtvec,
                None,
                Float3Instr(
                    test_params.prng.choice(Float3Instrs), frd, frs1, frs2, rm, False
                ),
                old_privilege,
            )

    if old_privilege == PrivLvl.Machine:
        if (
            "kronos" in test_params.design_name
            and not is_tolerate_kronos_readbadcsr()
            or "picorv32" in test_params.design_name
            and not is_tolerate_picorv32_readnonimplcsr()
        ):
            candidate_instructions = [
                TrapInstrWrapper(
                    is_mtvec,
                    None,
                    SimpleIllegalInstr(is_mtvec, old_privilege),
                    old_privilege,
                ),
            ]
        else:
            rd = corestate.intregpickstate.pick_random_integer_reg()
            rs1 = corestate.intregpickstate.pick_random_integer_reg()
            candidate_instructions = [
                TrapInstrWrapper(
                    is_mtvec,
                    None,
                    SimpleIllegalInstr(is_mtvec, old_privilege),
                    old_privilege,
                ),
                TrapInstrWrapper(
                    is_mtvec,
                    None,
                    CSRRegInstr("csrrw", rd, rs1, 0xCCA),
                    old_privilege,
                ),
            ]
    elif old_privilege == PrivLvl.Supervisor:
        rd = corestate.intregpickstate.pick_random_integer_reg()
        rs1 = corestate.intregpickstate.pick_random_integer_reg()
        csr = test_params.prng.choice(INTERESTING_CSRS_INACCESSIBLE_FROM_SUPERVISOR)
        candidate_instructions = [
            TrapInstrWrapper(
                is_mtvec,
                None,
                PrivDescentInstr(True, old_privilege),
                old_privilege,
            ),
            TrapInstrWrapper(
                is_mtvec,
                None,
                CSRRegInstr("csrrw", rd, rs1, csr),
                old_privilege,
            ),
        ]
    elif old_privilege == PrivLvl.User:
        rd = corestate.intregpickstate.pick_random_integer_reg()
        rs1 = corestate.intregpickstate.pick_random_integer_reg()
        csr = test_params.prng.choice(INTERESTING_CSRS_INACCESSIBLE_FROM_USER)
        candidate_instructions = [
            TrapInstrWrapper(
                is_mtvec,
                None,
                test_params.prng.choice(
                    [
                        PrivDescentInstr(True, old_privilege),
                        PrivDescentInstr(False, old_privilege),
                    ]
                ),
                old_privilege,
            ),
            TrapInstrWrapper(
                is_mtvec,
                None,
                CSRRegInstr("csrrw", rd, rs1, csr),
                old_privilege,
            ),
        ]
    else:
        raise Exception("Unknown privilege state: " + str(old_privilege))
    ret = test_params.prng.choice(candidate_instructions)
    return ret


# Has the side effect of consuming a tvec
def gen_exception_from_cause(
    corestate: "CoreState",
    test_params: "TestParams",
    exception_op_type: ExceptionCause,
):
    # Check for delegations
    if corestate.hartstate.privlvl == PrivLvl.Machine:
        is_mtvec = True
    else:
        is_mtvec = not (corestate.hartstate.medeleg & (1 << exception_op_type.value))

    if __debug__:
        if is_mtvec:
            assert corestate.hartstate.is_mtvec_populated
        else:
            assert corestate.hartstate.is_stvec_populated

    old_privilege = corestate.hartstate.privlvl
    if is_mtvec:
        corestate.hartstate.is_mepc_populated = False
        corestate.hartstate.mstatus_mpp = corestate.hartstate.privlvl
        corestate.hartstate.is_mtvec_populated = False
        corestate.hartstate.privlvl = PrivLvl.Machine
        corestate.hartstate.mstatus_mpie = corestate.hartstate.mstatus_mie
        corestate.hartstate.mstatus_mie = False
        if "smdbltrp" in test_params.design_extentions:
            corestate.hartstate.mstatus_mdt = True
    else:
        corestate.hartstate.is_sepc_populated = False
        corestate.hartstate.mstatus_spp = corestate.hartstate.privlvl
        corestate.hartstate.is_stvec_populated = False
        corestate.hartstate.privlvl = PrivLvl.Supervisor

    # Generate depending on the exception type.
    match exception_op_type:
        case ExceptionCause.ID_INSTR_ADDR_MISALIGNED:
            # TODO that is not true
            if __debug__:
                assert not test_params.c_ext_support, (
                    "Compressed instructions are supported, so no instruction address misalignment can occur."
                )
            # The instruction misalignment will always be 2 bytes, because CF
            # instructions have a granularity of 2 bytes.
            # Select the address to load. We care about blacklisting, in the
            # (erroneous) case where the data would have some influence.
            misaligned_tgt_addr = (
                test_params.prng.randrange(0, (test_params.memsize - 1) // 4) * 4 + 2
            )  # -1 because we dont want to have an access fault but an instruction misaligned fault here.
            if __debug__:
                assert misaligned_tgt_addr % 4 == 2
                assert misaligned_tgt_addr >= 0
                assert misaligned_tgt_addr + 4 < test_params.memsize
            # Find out the address of the instruction to be created, to make the relative jump
            jal_addr = corestate.bb_start_addrs[-1] + 4 * len(
                corestate.basic_blocks
            )  # NOC
            return TrapInstrWrapper(
                is_mtvec,
                None,
                JALInstr("jal", IntReg.zero, misaligned_tgt_addr - jal_addr),
                old_privilege,
            )
        case ExceptionCause.ID_INSTR_ACCESS_FAULT:
            raise NotImplementedError("ID_INSTR_ACCESS_FAULT not yet supported")
        case ExceptionCause.ID_ILLEGAL_INSTRUCTION:
            return _pick_illegal_instruction(
                is_mtvec, corestate, test_params, old_privilege
            )
        case ExceptionCause.ID_BREAKPOINT:
            corestate.rocket_minsret_innacurate = (
                "rocket" in test_params.design_name
                and not is_tolerate_rocket_minstret()
            )  # rocket has minstret inaccurate because of ecall/ebreak
            return TrapInstrWrapper(
                is_mtvec, None, EcallEbreakInstr("ebreak"), old_privilege
            )
        case ExceptionCause.ID_LOAD_ADDR_MISALIGNED:
            if __debug__:
                assert corestate.intregpickstate.exists_reg_in_state(
                    IntRegState.CONSUMED
                )
            rs1 = corestate.intregpickstate.pick_reg_in_state(IntRegState.CONSUMED)
            producer_id = corestate.intregpickstate.get_producer_id(rs1)
            corestate.intregpickstate.set_regstate(rs1, IntRegState.FREE)
            return MisalignedMemInstr(
                is_mtvec, corestate, test_params, True, old_privilege, rs1, producer_id
            )
        case ExceptionCause.ID_LOAD_ACCESS_FAULT:
            raise NotImplementedError("ID_LOAD_ACCESS_FAULT not yet supported")
        case ExceptionCause.ID_STORE_AMO_ADDR_MISALIGNED:
            if __debug__:
                assert corestate.intregpickstate.exists_reg_in_state(
                    IntRegState.CONSUMED
                )
            rs1 = corestate.intregpickstate.pick_reg_in_state(IntRegState.CONSUMED)
            producer_id = corestate.intregpickstate.get_producer_id(rs1)
            corestate.intregpickstate.set_regstate(rs1, IntRegState.FREE)
            return MisalignedMemInstr(
                is_mtvec, corestate, test_params, False, old_privilege, rs1, producer_id
            )
        case ExceptionCause.ID_STORE_AMO_ACCESS_FAULT:
            raise NotImplementedError("ID_STORE_AMO_ACCESS_FAULT not yet supported")
        case ExceptionCause.ID_ENVIRONMENT_CALL_FROM_U_MODE:
            if __debug__:
                assert old_privilege == PrivLvl.User
            corestate.rocket_minsret_innacurate = (
                "rocket" in test_params.design_name
                and not is_tolerate_rocket_minstret()
            )  # rocket has minstret inaccurate because of ecall/ebreak
            return TrapInstrWrapper(
                is_mtvec, None, EcallEbreakInstr("ecall"), old_privilege
            )
        case ExceptionCause.ID_ENVIRONMENT_CALL_FROM_S_MODE:
            if __debug__:
                assert old_privilege == PrivLvl.Supervisor
            corestate.rocket_minsret_innacurate = (
                "rocket" in test_params.design_name
                and not is_tolerate_rocket_minstret()
            )  # rocket has minstret inaccurate because of ecall/ebreak
            return TrapInstrWrapper(
                is_mtvec, None, EcallEbreakInstr("ecall"), old_privilege
            )
        case ExceptionCause.ID_ENVIRONMENT_CALL_FROM_M_MODE:
            if __debug__:
                assert old_privilege == PrivLvl.Machine
            corestate.rocket_minsret_innacurate = (
                "rocket" in test_params.design_name
                and not is_tolerate_rocket_minstret()
            )  # rocket has minstret inaccurate because of ecall/ebreak
            return TrapInstrWrapper(
                is_mtvec, None, EcallEbreakInstr("ecall"), old_privilege
            )
        case ExceptionCause.ID_INSTRUCTION_PAGE_FAULT:
            raise NotImplementedError("ID_INSTRUCTION_PAGE_FAULT not yet supported")
        case ExceptionCause.ID_LOAD_PAGE_FAULT:
            raise NotImplementedError("ID_LOAD_PAGE_FAULT not yet supported")
        case ExceptionCause.ID_STORE_AMO_PAGE_FAULT:
            raise NotImplementedError("ID_STORE_AMO_PAGE_FAULT not yet supported")


###
# Exposed functions
###


def gen_exception_instr(corestate: "CoreState", test_params: "TestParams"):
    """
    Randomly picks an exception for the current design
    The exception will consume a xTVEC register
    """
    has_consumed_reg = corestate.intregpickstate.exists_reg_in_state(
        IntRegState.CONSUMED
    )
    exceptionweights = test_params.exceptionoppickweights
    exception_op_type = _gen_next_exceptionoptype(
        corestate, test_params, exceptionweights, has_consumed_reg
    )
    return gen_exception_from_cause(corestate, test_params, exception_op_type)
