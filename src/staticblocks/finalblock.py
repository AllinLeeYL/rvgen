# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


# This module defines the final block.

from riscv import (
    CSR,
    IntReg,
    FloatReg,
    ILEN,
    Ordering,
    FenceOrdering,
    BYTES_ALLIGN_4,
)
from params import (
    RELOCATOR_REG,
    RDEP_MASK_REG,
    MAX_NUM_PICKABLE_INTREGS,
    MAX_NUM_PICKABLE_FLOATREGS,
    FPU_STATE_BITS_REG,
    TERMSIG,
    SEMAPHORE_1,
    BARRIER_PHASE,
    TOHOST_ADDR,
    CORESYNC_ADDR_REG,
)
from instgen import (
    ImmRdInstr,
    RegImmInstr,
    IntStoreInstr,
    IntLoadInstr,
    FloatIntRd1Instr,
    JALInstr,
    JALRInstr,
    RawDataWord,
    CSRRegInstr,
    R12DInstr,
    BranchInstr,
    AmoInstr,
    WfiInstr,
    FenceInstr,
)
from asm import li_into_reg
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from states import CoreState, FuzzerState
    from params import TestParams


MAX_N_INSTR = 29 + MAX_NUM_PICKABLE_INTREGS + MAX_NUM_PICKABLE_FLOATREGS
MAX_FINAL_BLOCK_SIZE = MAX_N_INSTR * ILEN


def _alloc_final_basic_block(fuzzerstate: "FuzzerState"):
    """Allocates memory for the final basic block."""
    fuzzerstate.final_bb_base_addr = fuzzerstate.memstate.gen_random_free_addr(
        BYTES_ALLIGN_4, MAX_FINAL_BLOCK_SIZE, 0, fuzzerstate.memstate.memsize
    )
    if fuzzerstate.final_bb_base_addr is None:
        return False
    fuzzerstate.memstate.alloc_mem_range(
        fuzzerstate.get_final_bb_base_addr(), MAX_FINAL_BLOCK_SIZE
    )
    return True


def gen_finalblock(
    fuzzerstate: "FuzzerState",
    test_params: "TestParams",
    is_rtl: bool = False,
):
    """Generate the instruction stream for the final block. The instructions
    will first write the content of regiosters, so we can retrieve them from
    the simulation verbose output. The, we write 1 to the TOHOST address to
    terminate simulation

    If we are in a multicore scenario, we choose one core which polls a counter,
    and the others increment it.

    We use convertion instr to get fpregs
    """

    final_bb_instr = []
    is_rv64 = test_params.is_rv64
    design_has_fpu = test_params.design_has_fpu
    int_store = "sd" if is_rv64 else "sw"
    int_load = "ld" if is_rv64 else "lw"
    # FIXME, this will not work for RV32 with double precision
    fp_mv = "fmv.x.d" if is_rv64 else "fmv.x.w"

    # The block is allocated during program generation only
    if not is_rtl and not _alloc_final_basic_block(fuzzerstate):
        return False

    final_bb_instr += [FenceInstr("fence", FenceOrdering.RW_RW)]
    # Update semaphore 2 to release potentially waiting cores. Work only for 2
    # cores and completly smashes the barrier
    if test_params.num_harts > 1:
        offset = BARRIER_PHASE - CORESYNC_ADDR_REG[1]
        final_bb_instr += [
            IntStoreInstr(
                "sw", CORESYNC_ADDR_REG[0], RDEP_MASK_REG[0], offset, 0, is_rv64
            ),
        ]

    # FIXME heterogenous, if a single core has an FPU
    if design_has_fpu:
        final_bb_instr += [
            CSRRegInstr("csrrw", IntReg.zero, FPU_STATE_BITS_REG[0], CSR.MSTATUS)
        ]

    if design_has_fpu:
        fpregs = list(FloatReg)
        for fpreg in fpregs:
            final_bb_instr += [
                FloatIntRd1Instr(fp_mv, FPU_STATE_BITS_REG[0], fpreg, is_rv64)
            ]

    intregs = list(IntReg)[:MAX_NUM_PICKABLE_INTREGS]
    for reg in intregs:
        final_bb_instr += [R12DInstr("add", RDEP_MASK_REG[0], IntReg.zero, reg)]

    ###
    # Stop request:
    # If there is one core, store directly, so we do not rely on
    # the Atomic extention that might not be implemented on single core
    # processors. Else, we wait for a counter to reach the number of cores.
    ###

    if test_params.num_harts > 1:
        offset = SEMAPHORE_1 - CORESYNC_ADDR_REG[1]
        final_bb_instr += [
            RegImmInstr("addi", IntReg.s11, CORESYNC_ADDR_REG[0], offset, is_rv64),
            RegImmInstr("addi", IntReg.t3, IntReg.zero, 1, is_rv64),
            AmoInstr("amoadd.w", IntReg.s10, IntReg.s11, IntReg.t3, Ordering.Seq, None),
            IntLoadInstr(int_load, IntReg.s10, IntReg.s11, 0, None, is_rv64),
            RegImmInstr("addi", IntReg.t3, IntReg.zero, test_params.num_harts, is_rv64),
            BranchInstr("beq", IntReg.t3, IntReg.s10, 3 * ILEN, True, is_rv64),
            # write and wait for non polling cores
            WfiInstr(),
            JALInstr("jal", IntReg.zero, -ILEN),
        ]

    if is_rtl:
        _gen_termination_sequence(
            test_params.design_name, final_bb_instr, int_store, is_rv64
        )
    else:
        _gen_termination_sequence("spike", final_bb_instr, int_store, is_rv64)

    if __debug__:
        assert len(final_bb_instr) * ILEN <= MAX_FINAL_BLOCK_SIZE, (
            f"size: {len(final_bb_instr)}, max: {MAX_N_INSTR}"
        )

    fuzzerstate.final_bb_instrs = final_bb_instr
    return True


def _gen_termination_sequence(
    design_name: str, final_bb_instr: list, int_store: str, is_rv64: bool
):
    match design_name:
        case "cheshire":
            lui_imm, _ = li_into_reg(0x3000000)  # cheshire scratch reg number 2
            final_bb_instr += [
                ImmRdInstr("lui", IntReg.t3, lui_imm, is_rv64),
                RegImmInstr("addi", IntReg.s10, IntReg.zero, TERMSIG, is_rv64),
                IntStoreInstr(int_store, IntReg.t3, IntReg.s10, 8, None, is_rv64),
                JALInstr("jal", IntReg.zero, 0),  # Loop until term
            ]
        case "xiangshan":
            final_bb_instr += [
                ImmRdInstr("lui", IntReg.a0, 0, is_rv64),
                RawDataWord(0x0005006B),  # xstrap rs0(a0)
                JALInstr("jal", IntReg.zero, 0),
            ]
        case "naxriscv":
            offset_val = TOHOST_ADDR - RELOCATOR_REG[1]
            lui_imm, _ = li_into_reg(offset_val)
            # Jump to pass symbol addr (same as tohost)
            final_bb_instr += [
                ImmRdInstr("lui", IntReg.t3, lui_imm, is_rv64),
                R12DInstr("add", IntReg.t3, IntReg.t3, RELOCATOR_REG[0]),
                JALRInstr("jalr", IntReg.zero, IntReg.t3, 0, None, is_rv64),
            ]
        case "vexiiriscv":
            offset_val = TOHOST_ADDR - RELOCATOR_REG[1]
            lui_imm, _ = li_into_reg(offset_val)
            # Jump to pass symbol addr (same as tohost)
            final_bb_instr += [
                ImmRdInstr("lui", IntReg.t3, lui_imm, is_rv64),
                R12DInstr("add", IntReg.t3, IntReg.t3, RELOCATOR_REG[0]),
                JALRInstr("jalr", IntReg.zero, IntReg.t3, 0, None, is_rv64),
            ]
        case "milesan-dualboom":
            # write at address 0x6000000
            lui_imm, addi_imm = li_into_reg(0x60000000)
            final_bb_instr += [
                ImmRdInstr("lui", IntReg.t3, lui_imm, is_rv64),
                RegImmInstr("addi", IntReg.t3, IntReg.t3, addi_imm, is_rv64),
                R12DInstr("add", IntReg.t3, IntReg.t3, RELOCATOR_REG[0]),
                IntStoreInstr("sd", IntReg.t3, IntReg.zero, 0, None, is_rv64),
            ]
        case _:
            offset_val = TOHOST_ADDR - RELOCATOR_REG[1]
            lui_imm, _ = li_into_reg(offset_val)
            final_bb_instr += [
                ImmRdInstr("lui", IntReg.t3, lui_imm, is_rv64),
                R12DInstr("add", IntReg.t3, IntReg.t3, RELOCATOR_REG[0]),
                RegImmInstr("addi", IntReg.s10, IntReg.zero, TERMSIG, is_rv64),
                IntStoreInstr(int_store, IntReg.t3, IntReg.s10, 0, None, is_rv64),
                JALInstr("jal", IntReg.zero, -ILEN),  # Loop back to store tohost
            ]
