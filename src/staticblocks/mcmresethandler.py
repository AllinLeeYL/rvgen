# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


import runparams
from params import (
    CORESYNC_ADDR_REG,
    FPU_STATE_BITS_REG,
    MPP_BOTH_BITS_REG,
    BARRIER_COUNT,
    BARRIER_PHASE,
    MCM_RESET_REG,
    RELOCATOR_REG,
    RDEP_MASK_REG,
)
from riscv import IntReg, Ordering, ILEN
from asm import li_into_reg
from instgen import (
    R12DInstr,
    RegImmInstr,
    BranchInstr,
    IntLoadInstr,
    IntStoreInstr,
    AmoInstr,
    FenceInstr,
    FenceOrdering,
    JALRInstr,
    ImmRdInstr,
    JALInstr,
    RVInstr,
)
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from params import TestParams
    from states import FuzzerState


def gen_mcm_statereset_handler(fuzzerstate: "FuzzerState", test_params: "TestParams"):
    """Implements a minimally intrusive handler to reset the state of the
    MCM and reset the store addresses. The handler implement the following
    pseudo-code:

    adapted from a two phase barrier. Reusable barriers with core local data
    cannot be used, as we cannot have core local data since we do not know which
    core we are using so we use a sense reversing barrier

    typedef struct {
        volatile int count;
        volatile int phase;
    } barrier_t;

    void barrier(barrier_t *bar, int n_hart) {
        int my_phase = bar->phase;
        if (my_phase == 0xffff_ffff) return //safe exit
        _asm("fence rw, rw");
        if (__atomic_add_fetch(&bar->count, 1, __ATOMIC_SEQ_CST) == n_hart) {
            bar->count = 0;
            bar->phase = 1 - my_phase; // Toggle phase
        } else {
            while (bar->phase == my_phase) { /* spin */ }
        }
        _asm("fence rw, rw");
    }

    The handler is located at 0x80001000
    """

    def gen_handler_exit() -> list[RVInstr]:
        ret = []
        for addr in fuzzerstate.store_locations:
            lui_imm, addi_imm = li_into_reg(addr)
            # FIXME will not work for rv32 for double precision FPU
            store_instr = "sd" if is_rv64 else "sw"
            ret += [
                RegImmInstr("addi", tmp_relocator_r, IntReg.zero, 1, is_rv64),
                RegImmInstr("slli", tmp_relocator_r, tmp_relocator_r, 31, is_rv64),
                ImmRdInstr("lui", store_addr_r, lui_imm, is_rv64),
                RegImmInstr("addi", store_addr_r, store_addr_r, addi_imm, is_rv64),
                R12DInstr("add", store_addr_r, store_addr_r, tmp_relocator_r),
                IntStoreInstr(store_instr, store_addr_r, IntReg.zero, 0, None, is_rv64),
            ]

        ret += [
            # bar->count = 0;
            IntStoreInstr("sw", count_addr_r, IntReg.zero, 0, None, is_rv64),
            # bar->phase = !my_phase; // Toggle phase
            RegImmInstr("xori", phase_r, phase_r, 1, is_rv64),
            IntStoreInstr("sw", count_addr_r, phase_r, phase_offset, None, is_rv64),
        ]
        return ret

    coresync_addr = CORESYNC_ADDR_REG[1] - RELOCATOR_REG[1]
    is_rv64 = test_params.is_rv64
    count_offset = BARRIER_COUNT - CORESYNC_ADDR_REG[1]
    phase_offset = BARRIER_PHASE - BARRIER_COUNT
    count_addr_r = CORESYNC_ADDR_REG[0]
    amo_dec_r = FPU_STATE_BITS_REG[0]
    n_hart_r = amo_dec_r
    tmp_relocator_r = amo_dec_r
    old_hartcnt_r = MPP_BOTH_BITS_REG[0]
    curr_hartcnt = old_hartcnt_r
    global_phase = old_hartcnt_r
    store_addr_r = old_hartcnt_r
    phase_r = RELOCATOR_REG[0]
    reset_section = gen_handler_exit()

    # Fence before entering
    handler_instr = [FenceInstr("fence", FenceOrdering.RW_RW)]

    load_instr = "lwu" if is_rv64 else "lw"
    handler_instr += [
        # int my_phase = bar->phase;
        RegImmInstr("addi", count_addr_r, count_addr_r, count_offset, is_rv64),
        IntLoadInstr(load_instr, phase_r, count_addr_r, phase_offset, None, is_rv64),
        # if (my_phase == 0xffff_ffff) return //safe exit FIXME the phase is resetted when exit
        BranchInstr("bne", RDEP_MASK_REG[0], phase_r, 2 * ILEN, True, is_rv64, True),
        JALInstr("jal", IntReg.zero, ILEN * (len(reset_section) + 9), True),
        # if (atomic_add(sync_addr, 1) == n_hart) {
        RegImmInstr("addi", amo_dec_r, IntReg.zero, 1, is_rv64),
        AmoInstr(
            "amoadd.w", old_hartcnt_r, count_addr_r, amo_dec_r, Ordering.Seq, None
        ),
        RegImmInstr("addi", curr_hartcnt, old_hartcnt_r, 1, is_rv64),
        RegImmInstr("addi", n_hart_r, IntReg.zero, test_params.num_harts, is_rv64),
        BranchInstr("beq", n_hart_r, curr_hartcnt, ILEN * 4, True, is_rv64, True),
        # while (bar->phase == my_phase) { /* spin */ }
        IntLoadInstr("lw", global_phase, count_addr_r, phase_offset, None, is_rv64),
        BranchInstr("beq", global_phase, phase_r, -ILEN, True, is_rv64, True),
        JALInstr("jal", IntReg.zero, ILEN * (len(reset_section) + 1), True),
    ]

    # for (addr in store_addrs) *addr = 0; *sync_addr = 0
    handler_instr += reset_section

    # This section is executed by both cores
    lui_imm, addi_imm = li_into_reg(coresync_addr)
    handler_instr += [
        # reset dedicated registers used
        RegImmInstr("addi", RELOCATOR_REG[0], IntReg.zero, 1, is_rv64),
        RegImmInstr("slli", RELOCATOR_REG[0], RELOCATOR_REG[0], 31, is_rv64),
        ImmRdInstr("lui", FPU_STATE_BITS_REG[0], 0b110, is_rv64),
        RegImmInstr("srli", MPP_BOTH_BITS_REG[0], FPU_STATE_BITS_REG[0], 2, is_rv64),
        ImmRdInstr("lui", CORESYNC_ADDR_REG[0], lui_imm, is_rv64),
        RegImmInstr(
            "addi", CORESYNC_ADDR_REG[0], CORESYNC_ADDR_REG[0], addi_imm, is_rv64
        ),
        R12DInstr("add", CORESYNC_ADDR_REG[0], CORESYNC_ADDR_REG[0], RELOCATOR_REG[0]),
        # return to testcase
        FenceInstr("fence", FenceOrdering.RW_RW),
        JALRInstr("jalr", IntReg.zero, MCM_RESET_REG, 0, None, is_rv64),
    ]
    assert len(handler_instr) * ILEN <= 0x1000, "too many store addresses"
    fuzzerstate.mcm_state_rst_instrs = handler_instr
