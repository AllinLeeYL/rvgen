# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


from params import MCM_RESET_REG
from asm import li_into_reg
from riscv import IntReg
from instgen import *
from states import IntRegState, FloatRegState, SyncState
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from states import CoreState, FuzzerState
    from params import TestParams


def goto_mcm_reset_handler(
    fuzzerstate: "FuzzerState", corestate: "CoreState", is_rv64: bool, prng: Random
):
    """jumps to the mcm reset handler"""
    rs1 = corestate.intregpickstate.pick_int_outputreg_nonzero()
    lui_imm, addi_imm = li_into_reg(fuzzerstate.mcm_state_rst_base_addr)
    ret = [
        ImmRdInstr("lui", rs1, lui_imm, is_rv64),
        RegImmInstr("addi", rs1, rs1, addi_imm, is_rv64),
        R12DInstr("add", rs1, rs1, RELOCATOR_REG[0]),
        JALRInstr("jalr", MCM_RESET_REG, rs1, 0, None, is_rv64),
    ]
    corestate.reset_memop_count(prng)
    assert (
        fuzzerstate.can_take_mcm_handler()
        and fuzzerstate.sync_states[corestate.hartid] == SyncState.FREE
    )
    fuzzerstate.sync_states[corestate.hartid] = SyncState.MCM
    corestate.mcm_reset_locs.append((corestate.get_cur_coord()))
    return ret


# slli, srli can only shift 31/63 bits, not enough to zero out a register
def freeing_instr(corestate: "CoreState", test_params: "TestParams"):
    choices = []
    if corestate.intregpickstate.exists_reg_in_state(IntRegState.POLLUTED):
        choices.append("int")
    if (
        corestate.floatregpickstate.exists_reg_in_state(FloatRegState.POLLUTED)
        and corestate.hartstate.mstatus_fs != FpuState.Off
    ):
        choices.append("float")
    assert choices
    choice = test_params.prng.choice(choices)
    if choice == "int":
        return free_polluted_intreg(corestate, test_params)
    else:
        return free_polluted_floatingreg(corestate, test_params)


def free_polluted_intreg(corestate: "CoreState", test_params: "TestParams"):
    """Picks an instruction which will free a register in the POLLUTED state without
    loosing the dependency to a previous memory operations.
    """
    ALU = ["xor", "sub", "slt", "sltu"]
    MULDIV = ["mul", "mulh", "mulhu", "mulhsu", "rem", "remu"]
    MULDIV64 = ["mulw", "remw", "remuw"]
    NEED_ZERO_REG = ["mul", "mulw", "mulh", "mulhu", "mulhsu"]

    available_instr = ALU
    if test_params.design_has_muldiv:
        available_instr += MULDIV
        if test_params.is_rv64:
            available_instr += MULDIV64

    instr_str = test_params.prng.choice(available_instr)
    polluted_reg = corestate.intregpickstate.pick_reg_in_state(IntRegState.POLLUTED)
    corestate.intregpickstate.set_regstate(polluted_reg, IntRegState.ZEROED)
    if instr_str in NEED_ZERO_REG:
        rs2 = IntReg.zero
    else:
        rs2 = polluted_reg

    ret = R12DInstr(instr_str, polluted_reg, polluted_reg, rs2)
    return [ret]


def free_polluted_floatingreg(corestate: "CoreState", test_params: "TestParams"):
    """Picks an instruction which will free a register in the POLLUTED state without
    loosing the dependency to a previous memory operations. We make all polluted
    register into a NaN
    """

    polluted_reg = corestate.floatregpickstate.pick_reg_in_state(FloatRegState.POLLUTED)
    corestate.floatregpickstate.set_regstate(polluted_reg, FloatRegState.FREE)
    tmp_reg = corestate.floatregpickstate.pick_float_outputreg()
    rm = test_params.prng.choice([0, 1, 2, 3, 4, 7])
    fp_mv = "fmv.d.x" if test_params.is_rv64 else "fmv.w.x"  # FIXME
    ret = [
        FloatIntRs1Instr(fp_mv, tmp_reg, IntReg.zero, test_params.is_rv64),
        Float3Instr(
            "fdiv.s", polluted_reg, polluted_reg, tmp_reg, rm, test_params.is_rv64
        ),
    ]

    return ret


def create_syntactic_dep(corestate: "CoreState", prng: Random):
    """randomly creates a dependency between a zeroed regiser and an address
    register

    entangle with a addr reg -> addr dep
    entangle with a normal reg -> data dep
    entangle with a normal reg which is used for a cf instr -> control dep

    WARNING, floating point registers in the ZEROED state are actually NaN !
    so we may not use them here
    """
    alu_ops = ["xor", "sub", "or", "add", "sll", "srl", "sra"]
    instr = prng.choice(alu_ops)
    zero_reg = corestate.intregpickstate.pick_reg_in_state(IntRegState.ZEROED)

    if prng.random() < 0.6:
        addr_reg = prng.choice(corestate.addr_regs)
    else:
        addr_reg = prng.choice(corestate.intregpickstate.pickable_intregs)
    ret = [R12DInstr(instr, addr_reg, addr_reg, zero_reg)]
    return ret
