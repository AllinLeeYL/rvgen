# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


from random import Random
from typing import TYPE_CHECKING

import runparams
from instgen import (
    INSTRUCTION_IDS,
    PARAM_IS_SIGNED,
    PARAM_REGTYPE,
    PARAM_SIZES_BITS_32,
    PARAM_SIZES_BITS_64,
    AmoInstr,
    AmoInstrs,
    AmoStore,
    AmoStores,
    BranchInstr,
    BranchInstrs,
    FenceInstr,
    FenceInstrs,
    Float2Instr,
    Float2Instrs,
    Float3Instr,
    Float3Instrs,
    Float3NoRmInstr,
    Float3NoRmInstrs,
    Float4Instr,
    Float4Instrs,
    FloatIntRd1Instr,
    FloatIntRd1Instrs,
    FloatIntRd2Instr,
    FloatIntRd2Instrs,
    FloatIntRs1Instr,
    FloatIntRs1Instrs,
    FloatLoadInstr,
    FloatLoadInstrs,
    FloatStoreInstr,
    FloatStoreInstrs,
    FloatToIntInstr,
    FloatToIntInstrs,
    ImmRdInstr,
    ImmRdInstrs,
    IntLoadInstr,
    IntLoadInstrs,
    IntStoreInstr,
    IntStoreInstrs,
    IntToFloatInstr,
    IntToFloatInstrs,
    JALInstr,
    JALInstrs,
    JALRInstr,
    JALRInstrs,
    R12DInstr,
    R12DInstrs,
    RegImmInstr,
    RegImmInstrs,
)
from riscv import FenceOrdering, IntReg, Ordering
from states import FloatRegState, IntRegState

if TYPE_CHECKING:
    from states import CoreState, FuzzerState

# This module creates an instruction from ..its instruction string, and some state
# which will condition which registers and immediates will be picked, and with
# which probability.

###
# Utility functions
###


def _gen_random_imm(instr_str: str, is_rv64: bool, prng: Random):
    if __debug__:
        assert PARAM_REGTYPE[INSTRUCTION_IDS[instr_str]][-1] == ""
    if is_rv64:
        imm_width = PARAM_SIZES_BITS_64[INSTRUCTION_IDS[instr_str]][-1]
    else:
        imm_width = PARAM_SIZES_BITS_32[INSTRUCTION_IDS[instr_str]][-1]
    if PARAM_IS_SIGNED[INSTRUCTION_IDS[instr_str]][-1]:
        left_bound = -(1 << (imm_width - 1))
        right_bound = 1 << (imm_width - 1)
    else:
        left_bound = 0
        right_bound = 1 << imm_width
    return prng.randrange(left_bound, right_bound)


# Random rounding modes
def gen_random_rounding_mode(prng: Random):
    """generates a random rounding mode"""
    return prng.sample([0, 1, 2, 3, 4, 7], 1)[0]


###
# Functions for creation by CFInstrClass
###

# Integer instructions


def _create_r12dinstr(instr_str: str, corestate: "CoreState", iscompressed: bool):
    rs1, rs2 = tuple(corestate.intregpickstate.pick_int_inputregs(2))
    rd = corestate.intregpickstate.pick_int_outputreg()
    return R12DInstr(instr_str, rd, rs1, rs2, iscompressed)


def _create_immrdinstr(
    instr_str: str,
    corestate: "CoreState",
    is_rv64: bool,
    iscompressed: bool,
    prng: Random,
):
    rd = corestate.intregpickstate.pick_int_outputreg()
    imm = _gen_random_imm(instr_str, is_rv64, prng)
    return ImmRdInstr(instr_str, rd, imm, is_rv64, iscompressed)


def _create_regimminstr(
    instr_str: str,
    corestate: "CoreState",
    is_rv64: bool,
    iscompressed: bool,
    prng: Random,
):
    rs1 = corestate.intregpickstate.pick_int_inputreg()
    rd = corestate.intregpickstate.pick_int_outputreg()
    imm = _gen_random_imm(instr_str, is_rv64, prng)
    return RegImmInstr(instr_str, rd, rs1, imm, is_rv64, iscompressed)


def _create_branchinstr(
    instr_str: str,
    corestate: "CoreState",
    prng: Random,
    curr_addr: int,
    is_rv64: bool,
    iscompressed: bool,
):
    rs1, rs2 = corestate.intregpickstate.pick_int_inputregs(2)
    plan_taken = corestate.curr_branch_taken
    if plan_taken:
        imm = corestate.get_next_bb_addr() - curr_addr
    else:
        # send to a random location. We can also loop infinitly TODO
        imm = _gen_random_imm(instr_str, is_rv64, prng)

    # print('New imm', hex(imm), flush=True)
    return BranchInstr(instr_str, rs1, rs2, imm, plan_taken, is_rv64, iscompressed)


def _create_jalinstr(
    instr_str: str,
    corestate: "CoreState",
    curr_addr: int,
    iscompressed: bool,
):
    rd = corestate.intregpickstate.pick_int_outputreg()
    imm = corestate.get_next_bb_addr() - curr_addr
    return JALInstr(instr_str, rd, imm, iscompressed)


def _create_jalrinstr(
    instr_str: str, corestate: "CoreState", is_rv64: bool, iscompressed: bool
):
    rs1 = corestate.intregpickstate.pick_reg_in_state(IntRegState.CONSUMED)
    producer_id = corestate.intregpickstate.get_producer_id(rs1)
    corestate.intregpickstate.set_regstate(rs1, IntRegState.FREE)
    rd = corestate.intregpickstate.pick_int_outputreg()
    imm = 0
    if __debug__:
        assert producer_id > 0
    return JALRInstr(instr_str, rd, rs1, imm, producer_id, is_rv64, iscompressed)


def _create_fenceinstr(instr_str: str, iscompressed: bool, prng: Random):
    f_ord = list(FenceOrdering)
    fence_ordering = prng.choice(f_ord)
    return FenceInstr(instr_str, fence_ordering, iscompressed)


def _create_intloadinstr(
    instr_str: str,
    corestate: "CoreState",
    is_rv64: bool,
    iscompressed: bool,
    prng: Random,
):
    corestate.add_memop()  # Register the memory operation
    if runparams.USE_ADDR_REGS:
        addr = prng.choice(list(corestate.addr_reg_pairs.keys()))
        rs1 = corestate.addr_reg_pairs[addr][0]
        imm = corestate.addr_reg_pairs[addr][1]
        producer_id = None
    else:
        addr = None
        rs1 = corestate.intregpickstate.pick_reg_in_state(IntRegState.CONSUMED)
        producer_id = corestate.intregpickstate.get_producer_id(rs1)
        if __debug__:
            assert producer_id > 0
        corestate.intregpickstate.set_regstate(rs1, IntRegState.FREE)
        imm = 0
    rd = corestate.intregpickstate.pick_int_outputreg_nonzero()
    if runparams.FUZZ_MCM:
        corestate.intregpickstate.set_regstate(rd, IntRegState.POLLUTED)
    return IntLoadInstr(
        instr_str, rd, rs1, imm, producer_id, is_rv64, iscompressed, addr
    )


def _create_intstoreinstr(
    instr_str: str,
    corestate: "CoreState",
    is_rv64: bool,
    iscompressed: bool,
    prng: Random,
    fuzzerstate: "FuzzerState",
):
    corestate.add_memop()  # Register the memory operation
    if runparams.USE_ADDR_REGS:
        addr = prng.choice(list(corestate.addr_reg_pairs.keys()))
        rs1 = corestate.addr_reg_pairs[addr][0]
        imm = corestate.addr_reg_pairs[addr][1]
        producer_id = None
    else:
        addr = None
        rs1 = corestate.intregpickstate.pick_reg_in_state(IntRegState.CONSUMED)
        producer_id = corestate.intregpickstate.get_producer_id(rs1)
        if __debug__:
            assert producer_id > 0
        corestate.intregpickstate.set_regstate(rs1, IntRegState.FREE)
        imm = 0

    # use unique value when using verify memory trace
    if runparams.VERIFY_MEMORY_TRACE:
        # FIXME we can remove the addi, we do not need independant values. Once
        # the addi is removed, we can use both the address and conused methods
        # safely. Yet, we still have to adapt the litmus translation.
        assert runparams.USE_ADDR_REGS  # or, we could overwrite the consumed reg
        rs2 = corestate.intregpickstate.pick_int_outputreg_nonzero()
        ret = [
            RegImmInstr(
                "addi",
                rs2,
                IntReg.zero,
                fuzzerstate.curr_store_val % (1 << 11),
                is_rv64,
            ),
            IntStoreInstr(
                instr_str, rs1, rs2, imm, producer_id, is_rv64, iscompressed, addr
            ),
        ]
        # update store value
        fuzzerstate.curr_store_val += 1
    else:
        rs2 = corestate.intregpickstate.pick_int_inputreg_nonzero()
        ret = [
            IntStoreInstr(
                instr_str, rs1, rs2, imm, producer_id, is_rv64, iscompressed, addr
            )
        ]

    return ret


# Floating-point instructions


def _create_floatloadinstr(
    instr_str: str,
    corestate: "CoreState",
    is_rv64: bool,
    iscompressed: bool,
    prng: Random,
):
    corestate.add_memop()  # Register the memory operation
    if runparams.USE_ADDR_REGS:
        addr = prng.choice(list(corestate.addr_reg_pairs.keys()))
        rs1 = corestate.addr_reg_pairs[addr][0]
        imm = corestate.addr_reg_pairs[addr][1]
        producer_id = None
    else:
        addr = None
        rs1 = corestate.intregpickstate.pick_reg_in_state(IntRegState.CONSUMED)
        producer_id = corestate.intregpickstate.get_producer_id(rs1)
        if __debug__:
            assert producer_id > 0
        corestate.intregpickstate.set_regstate(rs1, IntRegState.FREE)
        imm = 0
    frd = corestate.floatregpickstate.pick_float_outputreg()
    if runparams.FUZZ_MCM:
        corestate.floatregpickstate.set_regstate(frd, FloatRegState.POLLUTED)
    return FloatLoadInstr(
        instr_str, frd, rs1, imm, producer_id, is_rv64, iscompressed, addr
    )


def _create_floatstoreinstr(
    instr_str: str,
    corestate: "CoreState",
    is_rv64: bool,
    iscompressed: bool,
    prng: Random,
):
    corestate.add_memop()  # Register the memory operation
    if runparams.USE_ADDR_REGS:
        addr = prng.choice(list(corestate.addr_reg_pairs.keys()))
        rs1 = corestate.addr_reg_pairs[addr][0]
        imm = corestate.addr_reg_pairs[addr][1]
        producer_id = None
    else:
        addr = None
        rs1 = corestate.intregpickstate.pick_reg_in_state(IntRegState.CONSUMED)
        producer_id = corestate.intregpickstate.get_producer_id(rs1)
        if __debug__:
            assert producer_id > 0
        corestate.intregpickstate.set_regstate(rs1, IntRegState.FREE)
        imm = 0

    frs2 = corestate.floatregpickstate.pick_float_inputreg()
    return FloatStoreInstr(
        instr_str, rs1, frs2, imm, producer_id, is_rv64, iscompressed, addr
    )


def _create_floattointinstr(
    instr_str: str, corestate: "CoreState", iscompressed: bool, prng: Random
):
    rm = gen_random_rounding_mode(prng)
    frs1 = corestate.floatregpickstate.pick_float_inputreg()
    rd = corestate.intregpickstate.pick_int_outputreg()
    return FloatToIntInstr(instr_str, rd, frs1, rm, iscompressed)


def _create_inttofloatinstr(
    instr_str: str, corestate: "CoreState", iscompressed: bool, prng: Random
):
    rm = gen_random_rounding_mode(prng)
    rs1 = corestate.intregpickstate.pick_int_inputreg()
    frd = corestate.floatregpickstate.pick_float_outputreg()
    return IntToFloatInstr(instr_str, frd, rs1, rm, iscompressed)


def _create_float4instr(
    instr_str: str, corestate: "CoreState", iscompressed: bool, prng: Random
):
    rm = gen_random_rounding_mode(prng)
    frs1, frs2, frs3 = tuple(corestate.floatregpickstate.pick_float_inputregs(3))
    frd = corestate.floatregpickstate.pick_float_outputreg()
    return Float4Instr(instr_str, frd, frs1, frs2, frs3, rm, iscompressed)


def _create_float3instr(
    instr_str: str, corestate: "CoreState", iscompressed: bool, prng: Random
):
    rm = gen_random_rounding_mode(prng)
    frs1, frs2 = tuple(corestate.floatregpickstate.pick_float_inputregs(2))
    frd = corestate.floatregpickstate.pick_float_outputreg()
    return Float3Instr(instr_str, frd, frs1, frs2, rm, iscompressed)


def _create_float3norminstr(instr_str: str, corestate: "CoreState", iscompressed: bool):
    frs1, frs2 = tuple(corestate.floatregpickstate.pick_float_inputregs(2))
    frd = corestate.floatregpickstate.pick_float_outputreg()
    return Float3NoRmInstr(instr_str, frd, frs1, frs2, iscompressed)


def _create_float2instr(
    instr_str: str, corestate: "CoreState", iscompressed: bool, prng: Random
):
    rm = gen_random_rounding_mode(prng)
    frs1 = corestate.floatregpickstate.pick_float_inputreg()
    frd = corestate.floatregpickstate.pick_float_outputreg()
    return Float2Instr(instr_str, frd, frs1, rm, iscompressed)


def _create_floatintrd2instr(
    instr_str: str, corestate: "CoreState", iscompressed: bool
):
    frs1, frs2 = corestate.floatregpickstate.pick_float_inputregs(2)
    rd = corestate.intregpickstate.pick_int_outputreg()
    return FloatIntRd2Instr(instr_str, rd, frs1, frs2, iscompressed)


def _create_floatintrd1instr(
    instr_str: str, corestate: "CoreState", iscompressed: bool
):
    frs1 = corestate.floatregpickstate.pick_float_inputreg()
    rd = corestate.intregpickstate.pick_int_outputreg()
    return FloatIntRd1Instr(instr_str, rd, frs1, iscompressed)


def _create_floatintrs1instr(
    instr_str: str, corestate: "CoreState", iscompressed: bool
):
    frd = corestate.floatregpickstate.pick_float_outputreg()
    rs1 = corestate.intregpickstate.pick_int_inputreg()
    return FloatIntRs1Instr(instr_str, frd, rs1, iscompressed)


##
# Atomics
##


def _create_amoinstr(
    instr_str: str,
    corestate: "CoreState",
    prng: Random,
    is_rv64: bool,
    fuzzerstate: "FuzzerState",
):
    corestate.add_memop()  # Register the memory operation
    if runparams.USE_ADDR_REGS:
        addr = prng.choice(list(corestate.addr_reg_pairs.keys()))
        # even if the imm is ignored, the address remains valid
        rs1 = corestate.addr_reg_pairs[addr][0]
        # Ignore offset
        addr -= corestate.addr_reg_pairs[addr][1]
        assert addr in corestate.addr_reg_pairs
        producer_id = None
    else:
        addr = None
        rs1 = corestate.intregpickstate.pick_reg_in_state(IntRegState.CONSUMED)
        producer_id = corestate.intregpickstate.get_producer_id(rs1)
        if __debug__:
            assert producer_id > 0
        corestate.intregpickstate.set_regstate(rs1, IntRegState.FREE)
        imm = 0

    if runparams.FUZZ_MCM:
        rd, rs2 = corestate.intregpickstate.pick_int_outputregs_nonzero(2)
    else:
        rd = IntReg.zero
        rs2 = corestate.intregpickstate.pick_int_inputreg()
    ordering = prng.choice(list(Ordering))
    if runparams.FUZZ_MCM:
        corestate.intregpickstate.set_regstate(rd, IntRegState.POLLUTED)

    # use unique value when using verify memory trace
    if runparams.VERIFY_MEMORY_TRACE:
        assert runparams.USE_ADDR_REGS  # or, we could overwrite the conumed reg
        ret = [
            RegImmInstr(
                "addi",
                rs2,
                IntReg.zero,
                fuzzerstate.curr_store_val % (1 << 11),
                is_rv64,
            ),
            AmoInstr(instr_str, rd, rs1, rs2, ordering, producer_id, dest_addr=addr),
        ]
        fuzzerstate.curr_store_val += 1
    else:
        ret = [AmoInstr(instr_str, rd, rs1, rs2, ordering, producer_id, dest_addr=addr)]
    return ret


###
# Exposed function
###


# The reservation in the MemState is already done ahead and should not be
# reiterated here.
# @param jalr_addr_reg: only meaningful if a jalr is present (in the latter
# case, it should be the next instruction)
def create_instr(
    instr_str: str,
    corestate: "CoreState",
    fuzzerstate: "FuzzerState",
    prng: Random,
    curr_addr: int,
    is_rv64: bool,
    iscompressed: bool = False,
) -> list:
    """wrapper arround all instruction builder functions"""
    if __debug__:
        assert not iscompressed

    # Integer instructions
    if instr_str in R12DInstrs:
        ret = [_create_r12dinstr(instr_str, corestate, iscompressed)]
    elif instr_str in ImmRdInstrs:
        ret = [_create_immrdinstr(instr_str, corestate, is_rv64, iscompressed, prng)]
    elif instr_str in RegImmInstrs:
        ret = [_create_regimminstr(instr_str, corestate, is_rv64, iscompressed, prng)]
    elif instr_str in BranchInstrs:
        ret = [
            _create_branchinstr(
                instr_str, corestate, prng, curr_addr, is_rv64, iscompressed
            )
        ]
    elif instr_str in JALInstrs:
        ret = [_create_jalinstr(instr_str, corestate, curr_addr, iscompressed)]
    elif instr_str in JALRInstrs:
        ret = [_create_jalrinstr(instr_str, corestate, is_rv64, iscompressed)]
    elif instr_str in FenceInstrs:
        ret = [_create_fenceinstr(instr_str, iscompressed, prng)]
    elif instr_str in IntLoadInstrs:
        ret = [_create_intloadinstr(instr_str, corestate, is_rv64, iscompressed, prng)]
    elif instr_str in IntStoreInstrs:
        ret = _create_intstoreinstr(
            instr_str, corestate, is_rv64, iscompressed, prng, fuzzerstate
        )
    # Floating point instructions
    elif instr_str in FloatLoadInstrs:
        ret = [
            _create_floatloadinstr(instr_str, corestate, is_rv64, iscompressed, prng)
        ]
    elif instr_str in FloatStoreInstrs:
        ret = [
            _create_floatstoreinstr(instr_str, corestate, is_rv64, iscompressed, prng)
        ]
    elif instr_str in FloatToIntInstrs:
        ret = [_create_floattointinstr(instr_str, corestate, iscompressed, prng)]
    elif instr_str in IntToFloatInstrs:
        ret = [_create_inttofloatinstr(instr_str, corestate, iscompressed, prng)]
    elif instr_str in Float4Instrs:
        ret = [_create_float4instr(instr_str, corestate, iscompressed, prng)]
    elif instr_str in Float3Instrs:
        ret = [_create_float3instr(instr_str, corestate, iscompressed, prng)]
    elif instr_str in Float3NoRmInstrs:
        ret = [_create_float3norminstr(instr_str, corestate, iscompressed)]
    elif instr_str in Float2Instrs:
        ret = [_create_float2instr(instr_str, corestate, iscompressed, prng)]
    elif instr_str in FloatIntRd2Instrs:
        ret = [_create_floatintrd2instr(instr_str, corestate, iscompressed)]
    elif instr_str in FloatIntRd1Instrs:
        ret = [_create_floatintrd1instr(instr_str, corestate, iscompressed)]
    elif instr_str in FloatIntRs1Instrs:
        ret = [_create_floatintrs1instr(instr_str, corestate, iscompressed)]
    elif instr_str in AmoInstrs:
        ret = _create_amoinstr(instr_str, corestate, prng, is_rv64, fuzzerstate)
    else:
        raise ValueError(f"Unexpected instruction string: `{instr_str}`")

    return ret


def create_instr_stateloop(
    instr_str: str,
    corestate: "CoreState",
    fuzzerstate: "FuzzerState",
    prng: Random,
    curr_addr: int,
    is_rv64: bool,
) -> list:
    """Wrapper arround the cretae instruction fuction which ensure that the
    input registers are not picked as output registers. This ensure we do not
    break the state perservation property of the state preserving loop
    """
    corestate.intregpickstate.reset_recent_inputs()
    corestate.floatregpickstate.reset_recent_inputs()
    ret = create_instr(instr_str, corestate, fuzzerstate, prng, curr_addr, is_rv64)
    return ret
