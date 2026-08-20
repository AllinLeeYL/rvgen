# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


from copy import copy
from collections import defaultdict
from typing import TYPE_CHECKING
import runparams
from toleratebugs import (
    is_tolerate_cva6_fdivs_flags,
    is_tolerate_vexriscv_imprecise_fcvt,
    is_tolerate_vexriscv_fmin,
    is_tolerate_vexriscv_double_to_float,
    is_tolerate_vexriscv_dependent_single_precision,
    is_tolerate_vexriscv_dependent_fle_feq_ret1,
    is_tolerate_vexriscv_dependent_flt_ret0,
    is_tolerate_vexriscv_sqrt,
    is_tolerate_vexriscv_muldiv_conversion,
    is_tolerate_cva6_single_precision,
    is_tolerate_cva6_division,
)
from instgen import ISAInstrClass, INSTRS_BY_ISA_CLASS
from params import NUM_MIN_INPUTS

if TYPE_CHECKING:
    from params import TestParams
    from states import CoreState

# For a given ISAInstrClass, this module helps picking an instruction type.

###
# Helper functions
###

INSTRTYPE_INITIAL_RELATIVE_WEIGHTS = {
    # For the moment, give all instructions inside the same ISA class the same
    # appearance chance.
    curr_key: dict.fromkeys(curr_instrs, 1)
    for curr_key, curr_instrs in INSTRS_BY_ISA_CLASS.items()
}


# Useful for time to bug evaluation
def forbid_vexriscv_ops(instr_proba):
    instr_proba_ret = copy(instr_proba)
    # Double precision
    instr_proba_ret["fsgnj.d"] = 0
    instr_proba_ret["fsgnjn.d"] = 0
    instr_proba_ret["fsgnjx.d"] = 0
    instr_proba_ret["fnmadd.s"] = 0
    instr_proba_ret["fmadd.s"] = 0
    instr_proba_ret["fnmsub.s"] = 0
    instr_proba_ret["fmsub.s"] = 0

    # Single precision
    instr_proba_ret["fsgnj.s"] = 0
    instr_proba_ret["fsgnjn.s"] = 0
    instr_proba_ret["fsgnjx.s"] = 0
    instr_proba_ret["fnmadd.d"] = 0
    instr_proba_ret["fmadd.d"] = 0
    instr_proba_ret["fnmsub.d"] = 0
    instr_proba_ret["fmsub.d"] = 0

    if (
        is_tolerate_vexriscv_imprecise_fcvt()
        or is_tolerate_vexriscv_fmin()
        or is_tolerate_vexriscv_double_to_float()
        or is_tolerate_vexriscv_dependent_single_precision()
        or is_tolerate_vexriscv_dependent_fle_feq_ret1()
        or is_tolerate_vexriscv_dependent_flt_ret0()
        or is_tolerate_vexriscv_sqrt()
        or is_tolerate_vexriscv_muldiv_conversion()
    ):
        instr_proba_ret["fcvt.w.s"] = 0
        instr_proba_ret["fcvt.wu.s"] = 0
        instr_proba_ret["fmin.s"] = 0
        instr_proba_ret["fmax.s"] = 0
        instr_proba_ret["fsqrt.s"] = 0
        instr_proba_ret["fmul.s"] = 0
        instr_proba_ret["fmul.d"] = 0
        instr_proba_ret["fadd.s"] = 0
        instr_proba_ret["fsub.s"] = 0
        instr_proba_ret["fdiv.s"] = 0
        instr_proba_ret["fdiv.d"] = 0
        instr_proba_ret["flt.s"] = 0
        instr_proba_ret["fle.s"] = 0
        instr_proba_ret["feq.s"] = 0
        instr_proba_ret["fcvt.l.s"] = 0
        instr_proba_ret["fcvt.lu.s"] = 0
        instr_proba_ret["fcvt.s.w"] = 0
        instr_proba_ret["fcvt.s.wu"] = 0
        instr_proba_ret["fcvt.s.l"] = 0
        instr_proba_ret["fcvt.s.lu"] = 0
        instr_proba_ret["fcvt.d.s"] = 0
        instr_proba_ret["fmin.d"] = 0
        instr_proba_ret["fmax.d"] = 0

        if (
            is_tolerate_vexriscv_imprecise_fcvt()
            or is_tolerate_vexriscv_fmin()
            or is_tolerate_vexriscv_double_to_float()
            or is_tolerate_vexriscv_dependent_single_precision()
            or is_tolerate_vexriscv_dependent_fle_feq_ret1()
            or is_tolerate_vexriscv_dependent_flt_ret0()
            or is_tolerate_vexriscv_sqrt()
        ):
            instr_proba_ret["fcvt.w.s"] = instr_proba["fcvt.w.s"]
            instr_proba_ret["fcvt.wu.s"] = instr_proba["fcvt.wu.s"]
            instr_proba_ret["fcvt.l.s"] = instr_proba["fcvt.l.s"]
            instr_proba_ret["fcvt.lu.s"] = instr_proba["fcvt.lu.s"]
            instr_proba_ret["fcvt.d.s"] = instr_proba["fcvt.d.s"]

        if is_tolerate_vexriscv_imprecise_fcvt():
            instr_proba_ret["fcvt.s.w"] = instr_proba["fcvt.s.w"]
            instr_proba_ret["fcvt.s.wu"] = instr_proba["fcvt.s.wu"]
            instr_proba_ret["fcvt.s.l"] = instr_proba["fcvt.s.l"]
            instr_proba_ret["fcvt.s.lu"] = instr_proba["fcvt.s.lu"]
        if is_tolerate_vexriscv_fmin():
            instr_proba_ret["fmin.s"] = instr_proba["fmin.s"]
            instr_proba_ret["fmin.d"] = instr_proba["fmin.d"]
        if is_tolerate_vexriscv_double_to_float():
            instr_proba_ret["fcvt.d.s"] = instr_proba["fcvt.d.s"]
        if is_tolerate_vexriscv_dependent_single_precision():
            instr_proba_ret["fmul.s"] = instr_proba["fmul.s"]
            instr_proba_ret["fadd.s"] = instr_proba["fadd.s"]
            instr_proba_ret["fsub.s"] = instr_proba["fsub.s"]
            instr_proba_ret["fdiv.s"] = instr_proba["fdiv.s"]
        if is_tolerate_vexriscv_dependent_fle_feq_ret1():
            instr_proba_ret["fle.s"] = instr_proba["fle.s"]
            instr_proba_ret["fle.d"] = instr_proba["fle.d"]
            instr_proba_ret["feq.s"] = instr_proba["feq.s"]
            instr_proba_ret["feq.d"] = instr_proba["feq.d"]
        if is_tolerate_vexriscv_dependent_flt_ret0():
            instr_proba_ret["flt.s"] = instr_proba["flt.s"]
            instr_proba_ret["flt.d"] = instr_proba["flt.d"]
        if is_tolerate_vexriscv_sqrt():
            instr_proba_ret["fsqrt.s"] = instr_proba["fsqrt.s"]
            instr_proba_ret["fsqrt.d"] = instr_proba["fsqrt.d"]

        if is_tolerate_vexriscv_muldiv_conversion():
            instr_proba_ret["fmul.s"] = instr_proba["fmul.s"]
            instr_proba_ret["fmul.d"] = instr_proba["fmul.d"]
            instr_proba_ret["fdiv.s"] = instr_proba["fdiv.s"]
            instr_proba_ret["fdiv.d"] = instr_proba["fdiv.d"]

    return instr_proba_ret


###
# Exposed functions
###


# @param isaclass
# @return a RVInstr object or a placeholder object
def gen_next_instrstr_from_isaclass(
    isaclass: ISAInstrClass, test_params: "TestParams", corestate: "CoreState"
) -> str:
    # This global may prevent from ..copying the dict
    global INSTRTYPE_INITIAL_RELATIVE_WEIGHTS

    instr_proba = defaultdict(int, INSTRTYPE_INITIAL_RELATIVE_WEIGHTS[isaclass])

    # No floating point sign injection
    if "vexriscv" in test_params.design_name:
        instr_proba = forbid_vexriscv_ops(instr_proba)

    if test_params.design_name == "cva6":
        # Double precision
        instr_proba["fsqrt.d"] = 0
        if not is_tolerate_cva6_division():
            instr_proba["fdiv.d"] = 0

        # Single precision
        if not is_tolerate_cva6_single_precision():
            instr_proba["fsqrt.s"] = 0
            instr_proba["fdiv.s"] = 0
            instr_proba["fcvt.d.s"] = 0
            instr_proba["fcvt.s.d"] = 0
            instr_proba["fcvt.wu.d"] = 0

        if not is_tolerate_cva6_fdivs_flags():
            instr_proba["fdiv.s"] = 0

    # To facilitae value propaguation in the MCM calculation,
    # we make sure store are larger than loads
    if runparams.FUZZ_MCM and not runparams.VERIFY_MEMORY_TRACE:
        if isaclass in (
            ISAInstrClass.MEM,
            ISAInstrClass.MEM64,
        ):
            # disable small loads
            instr_proba["sb"] = 0
            instr_proba["sh"] = 0
            instr_proba["sw"] = 0
            # Adapt store instr proba
            store_proba = 2 # 2
            if test_params.is_rv64:
                instr_proba["sd"] = store_proba
            else:
                instr_proba["sw"] = store_proba
        if isaclass in (ISAInstrClass.MEMFPU, ISAInstrClass.MEMFPUD):
            if test_params.design_has_fpud:
                instr_proba["fsw"] = 0
            # Adapt store instr proba
            store_proba = 2
            if test_params.design_has_fpud:
                instr_proba["fsd"] = store_proba
            else:
                instr_proba["fsw"] = store_proba

    if runparams.VERIFY_MEMORY_TRACE:
        if isaclass == ISAInstrClass.AMO:
            instr_proba["amomaxu.w"] = 0
            instr_proba["amominu.w"] = 0
        if isaclass == ISAInstrClass.AMO64:
            instr_proba["amomaxu.d"] = 0
            instr_proba["amominu.d"] = 0
        if isaclass == ISAInstrClass.MEM:
            instr_proba["lbu"] = 0
            instr_proba["lhu"] = 0
            instr_proba["lb"] = 0
            instr_proba["sb"] = 0
            instr_proba["sh"] = 0

    if runparams.VERIFY_MEMORY_TRACE == "dat":
        if isaclass == ISAInstrClass.MEM:
            instr_proba["lb"] = 0
            instr_proba["lh"] = 0
            instr_proba["lbu"] = 0
            instr_proba["lhu"] = 0
            instr_proba["sb"] = 0
            instr_proba["sh"] = 0

    # lr/sc not yet supported, extra handling needed FIXME
    if isaclass == ISAInstrClass.AMO64:
        instr_proba["lr.d"] = 0
        instr_proba["sc.d"] = 0
    if isaclass == ISAInstrClass.AMO:
        instr_proba["lr.w"] = 0
        instr_proba["sc.w"] = 0

    # load consume an INPUT by polluting a reg
    too_little_inputs = corestate.intregpickstate.get_num_int_input() <= NUM_MIN_INPUTS
    if too_little_inputs and runparams.FUZZ_MCM:
        instr_proba["lb"] = 0
        instr_proba["lh"] = 0
        instr_proba["lw"] = 0
        instr_proba["lbu"] = 0
        instr_proba["lhu"] = 0
        instr_proba["lwu"] = 0
        instr_proba["ld"] = 0

    too_little_inputs = (
        corestate.floatregpickstate.get_num_float_input() <= NUM_MIN_INPUTS
    )
    if too_little_inputs and runparams.FUZZ_MCM:
        instr_proba["flw"] = 0
        instr_proba["fld"] = 0

    ret = None
    weights = list(instr_proba.values())
    ret = test_params.prng.choices(list(instr_proba.keys()), weights=weights)[0]
    assert ret != None
    return ret
