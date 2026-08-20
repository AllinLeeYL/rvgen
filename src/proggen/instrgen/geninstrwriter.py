# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only
from typing import TYPE_CHECKING
from proggen.util import create_instr
from params import (
    CORESYNC_ADDR_REG,
    RDEP_MASK_REG,
    RELOCATOR_REG,
    INSTR_BARRIERS_OFFSET,
)
from asm import li_into_reg
from instgen import (
    IntStoreInstr,
    FenceInstr,
    ImmRdInstr,
    RegImmInstr,
    R12DInstr,
    IntLoadInstr,
    BranchInstr,
    INSTRS_BY_ISA_CLASS,
    ISAInstrClass,
    RawDataWord,
)
from riscv import FenceOrdering, FpuState, ILEN, IntReg
from states import SyncState

if TYPE_CHECKING:
    from states import CoreState, FuzzerState
    from params import TestParams

SYNC_FLAG_VALUE = 0xFF


def wait_for_instr(
    test_params: "TestParams", fuzzerstate: "FuzzerState", corestate: "CoreState"
):
    """Waits for a ready signal, executes a fence.i and continues.
    stores a zero word where the instruction lands
    we need to pass the address of the instruction to the sender via the
    fuzzerstate. The address can be added before the elf generation
    We also pick the instruction here, and pass it to the sender, to get the
    correct register states
    """
    assert fuzzerstate.instr_write_gen_sequences is None

    # update sync state, core starts waiting for signal
    assert fuzzerstate.is_global_sync_state_free()
    fuzzerstate.sync_states[corestate.hartid] = SyncState.WRITE_INSTR

    rs2 = corestate.intregpickstate.pick_int_outputreg_nonzero()
    barrier_offset = INSTR_BARRIERS_OFFSET + corestate.hartid
    ret = [
        IntLoadInstr(
            "lbu",
            rs2,
            CORESYNC_ADDR_REG[0],
            barrier_offset,
            None,
            test_params.is_rv64,
            is_interrupt=True,
        ),
        RegImmInstr("addi", rs2, rs2, -SYNC_FLAG_VALUE, test_params.is_rv64),
        BranchInstr(
            "bne",
            rs2,
            IntReg.zero,
            -2 * ILEN,
            True,
            test_params.is_rv64,
            fixed_opcode=True,
        ),
        FenceInstr("fence", FenceOrdering.RW_RW),
        FenceInstr("fence.i", FenceOrdering.RW_RW),
    ]

    # transmit address
    instr_addr = corestate.get_current_addr() + len(ret) * ILEN

    # add placeholder and reset signal address to 0
    ret += [
        RawDataWord(0x0),
        IntStoreInstr(
            "sb",
            CORESYNC_ADDR_REG[0],
            IntReg.zero,
            barrier_offset,
            None,
            test_params.is_rv64,
            is_interrupt=True,
        ),
    ]

    # make instruction bytecode
    allowed_instrs = _get_allowed_instructions(test_params, corestate)
    instr_str = test_params.prng.choice(allowed_instrs)
    instr = create_instr(
        instr_str,
        corestate,
        fuzzerstate,
        test_params.prng,
        corestate.get_current_addr(),
        test_params.is_rv64,
    )[0]
    # instr bytecode
    instr_bytes = instr.gen_bytecode_int(is_spike_resolution=False)

    fuzzerstate.instr_write_gen_sequences = (instr_addr, instr_bytes, corestate.hartid)
    return ret


def write_foreign_instr(
    test_params: "TestParams", fuzzerstate: "FuzzerState", corestate: "CoreState"
):
    """Writes an instruction to a foreign core.
    Writes the instruction and send a ready signal
    """
    # update sync state, core starts waiting for signal
    assert not fuzzerstate.is_global_sync_state_free()
    assert fuzzerstate.instr_write_gen_sequences is not None

    instr_addr, instr_bytes, waiting_hartid = fuzzerstate.instr_write_gen_sequences
    addr_reg, instr_reg = corestate.intregpickstate.pick_int_outputregs_nonzero(2)
    barrier_offset = INSTR_BARRIERS_OFFSET + waiting_hartid

    # generate target address
    lui_imm, addi_imm = li_into_reg(instr_addr, do_check_bounds=False)
    ret = [
        ImmRdInstr("lui", addr_reg, lui_imm, test_params.is_rv64),
        RegImmInstr("addi", addr_reg, addr_reg, addi_imm, test_params.is_rv64),
        R12DInstr("add", addr_reg, addr_reg, RELOCATOR_REG[0]),
    ]

    # generate target instruction
    lui_imm, addi_imm = li_into_reg(instr_bytes, do_check_bounds=False)
    ret += [
        ImmRdInstr("lui", instr_reg, lui_imm, test_params.is_rv64),
        RegImmInstr("addi", instr_reg, instr_reg, addi_imm, test_params.is_rv64),
        R12DInstr("and", instr_reg, instr_reg, RDEP_MASK_REG[0]),
    ]

    # fence ensures the instruction store performs before the signaling
    ret += [
        IntStoreInstr(
            "sw", addr_reg, instr_reg, 0, None, test_params.is_rv64, is_interrupt=True
        ),
        FenceInstr("fence", FenceOrdering.W_W),
    ]

    # signal success and wait for ack
    ret += [
        IntStoreInstr(
            "sb",
            CORESYNC_ADDR_REG[0],
            RDEP_MASK_REG[0],
            barrier_offset,
            None,
            test_params.is_rv64,
            is_interrupt=True,
        ),
        IntLoadInstr(
            "lbu",
            instr_reg,
            CORESYNC_ADDR_REG[0],
            barrier_offset,
            None,
            test_params.is_rv64,
            is_interrupt=True,
        ),
        BranchInstr(
            "bne",
            instr_reg,
            IntReg.zero,
            -ILEN,
            True,
            test_params.is_rv64,
            fixed_opcode=True,
        ),
    ]

    fuzzerstate.sync_states[waiting_hartid] = SyncState.FREE
    fuzzerstate.instr_write_gen_sequences = None

    return ret


def _get_allowed_instructions(test_params: "TestParams", corestate: "CoreState"):
    """gen the set on instruction that can be generated"""
    allowed_instrs = INSTRS_BY_ISA_CLASS[ISAInstrClass.ALU]
    allowed_instrs += INSTRS_BY_ISA_CLASS[ISAInstrClass.MULDIV]
    if test_params.is_rv64:
        allowed_instrs += INSTRS_BY_ISA_CLASS[ISAInstrClass.ALU64]
        allowed_instrs += INSTRS_BY_ISA_CLASS[ISAInstrClass.MULDIV64]
    # is_fpu_activated = corestate.hartstate.mstatus_fs != FpuState.Off
    # if test_params.design_has_fpu and is_fpu_activated:
    #     allowed_instrs += INSTRS_BY_ISA_CLASS[ISAInstrClass.FPU]
    # if test_params.design_has_fpud and is_fpu_activated:
    #     allowed_instrs += INSTRS_BY_ISA_CLASS[ISAInstrClass.FPUD]
    # if test_params.is_rv64:
    #     if test_params.design_has_fpu and is_fpu_activated:
    #         allowed_instrs += INSTRS_BY_ISA_CLASS[ISAInstrClass.FPU64]
    #     if test_params.design_has_fpud and is_fpu_activated:
    #         allowed_instrs += INSTRS_BY_ISA_CLASS[ISAInstrClass.FPUD64]
    return allowed_instrs
