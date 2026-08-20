# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only
"""
This module defines the initial basic block of the program.
It initialises the CSR states and generates the initial value of all the
all-purpose and FPU registers
"""


from typing import TYPE_CHECKING
from math import log2
import runparams
from params import (
    RELOCATOR_REG,
    RDEP_MASK_REG,
    FPU_STATE_BITS_REG,
    MPP_BOTH_BITS_REG,
    BASIC_BLOCK_MIN_SPACE,
    MAX_NUM_PICKABLE_INTREGS,
    CORESYNC_ADDR_REG,
    SPP_BIT_VAL,
    MCM_RESET_REG,
    STRESS_SECTION_BASE,
)
from asm import li_into_reg
from toleratebugs import is_forbid_vexriscv_csrs
from instgen import (
    ImmRdInstr,
    RegImmInstr,
    R12DInstr,
    IntLoadInstr,
    IntStoreInstr,
    FloatLoadInstr,
    CSRRegInstr,
    CSRImmInstr,
    BranchInstr,
    FloatIntRs1Instr,
    FenceInstr,
    ISAInstrClass,
    JALRInstr,
    get_range_bits,
)
from riscv import (
    ILEN,
    DWORD_SIZE,
    BYTES_ALLIGN_4,
    MTIMER_OFFSET,
    SPIKE_CLINT_BASE,
    CSR,
    IntReg,
    FloatReg,
    FenceOrdering,
)

if TYPE_CHECKING:
    from states import FuzzerState, CoreState
    from params import TestParams
    from instgen import RVInstr


def _gen_dedicated_regs(instrs: "list[RVInstr]", is_rv64: bool, num_harts: int):
    shift_imm = int(log2(RELOCATOR_REG[1]))
    instrs += [
        RegImmInstr("addi", RELOCATOR_REG[0], IntReg.zero, 0x1, is_rv64),
        RegImmInstr("slli", RELOCATOR_REG[0], RELOCATOR_REG[0], shift_imm, is_rv64),
    ]
    if is_rv64:
        instrs += [
            RegImmInstr("addi", RDEP_MASK_REG[0], IntReg.zero, -1, is_rv64),
            RegImmInstr("srli", RDEP_MASK_REG[0], RDEP_MASK_REG[0], 32, is_rv64),
        ]

    if num_harts > 1:
        base_offset = CORESYNC_ADDR_REG[1] - RELOCATOR_REG[1]
        lui_imm, addi_imm = li_into_reg(base_offset)
        instrs += [
            ImmRdInstr("lui", CORESYNC_ADDR_REG[0], lui_imm, is_rv64),
            RegImmInstr(
                "addi", CORESYNC_ADDR_REG[0], CORESYNC_ADDR_REG[0], addi_imm, is_rv64
            ),
            R12DInstr(
                "add", CORESYNC_ADDR_REG[0], CORESYNC_ADDR_REG[0], RELOCATOR_REG[0]
            ),
        ]

    instrs += [
        # Create the dedicated regs
        ImmRdInstr("lui", FPU_STATE_BITS_REG[0], 0b110, is_rv64),
        RegImmInstr("srli", MPP_BOTH_BITS_REG[0], FPU_STATE_BITS_REG[0], 2, is_rv64),
    ]


def _init_multicore(instrs: "list[RVInstr]", is_rv64: bool, fuzzerstate: "FuzzerState"):
    # Clear Timer interrupt
    lui_imm = (SPIKE_CLINT_BASE + MTIMER_OFFSET) >> 12
    store_instr = "sd" if is_rv64 else "sw"
    fuzzerstate.mtimer_reset_idx = len(instrs)
    instrs.append(ImmRdInstr("lui", IntReg.ra, lui_imm, is_rv64))
    instrs.append(R12DInstr("and", IntReg.ra, IntReg.ra, RDEP_MASK_REG[0]))
    # Multiply harti id by 8
    instrs += [
        CSRRegInstr("csrrs", IntReg.gp, IntReg.zero, CSR.MHARTID),
        RegImmInstr("slli", IntReg.gp, IntReg.gp, 3, is_rv64),
        R12DInstr("add", IntReg.ra, IntReg.gp, IntReg.ra),
        RegImmInstr("addi", IntReg.sp, IntReg.zero, -1, is_rv64),
        # Store high value in timer comparer, to never trigger machine timer
        IntStoreInstr(store_instr, IntReg.ra, IntReg.sp, 0, None, is_rv64),
    ]


def _initialize_csrs(instrs: "list[RVInstr]", is_rv64: bool, test_params: "TestParams"):
    if not ("vexriscv" in test_params.design_name and is_forbid_vexriscv_csrs()):
        # Initialize to 0 for convinience (to avoid randdom initialization)
        instrs += [CSRRegInstr("csrrw", IntReg.zero, IntReg.zero, CSR.MSTATUS)]
        if test_params.s_mode_support:
            instrs += [
                CSRRegInstr("csrrw", IntReg.zero, IntReg.zero, CSR.MEDELEG),
                CSRRegInstr("csrrw", IntReg.zero, IntReg.zero, CSR.MIDELEG),
                CSRRegInstr("csrrw", IntReg.zero, IntReg.zero, CSR.STVEC),
            ]
        if test_params.design_name != "picorv32":
            instrs += [
                CSRRegInstr("csrrw", IntReg.zero, IntReg.zero, CSR.MTVEC),
            ]

    # We authorize all accesses through the PMP registers
    if not ("vexriscv" in test_params.design_name and is_forbid_vexriscv_csrs()):
        if test_params.pmp_support:
            instrs += [
                RegImmInstr("addi", IntReg.ra, IntReg.zero, 31, is_rv64),
                CSRRegInstr("csrrw", IntReg.zero, IntReg.ra, CSR.PMPCFG0),
            ]
            if is_rv64:
                instrs += [
                    RegImmInstr("addi", IntReg.ra, IntReg.zero, 1, is_rv64),
                    RegImmInstr("slli", IntReg.ra, IntReg.ra, 0x36, is_rv64),
                    RegImmInstr("addi", IntReg.ra, IntReg.ra, -1, is_rv64),
                    CSRRegInstr("csrrw", IntReg.zero, IntReg.ra, CSR.PMPADDR0),
                ]
            else:
                instrs += [
                    RegImmInstr("addi", IntReg.ra, IntReg.zero, -1, is_rv64),
                    CSRRegInstr("csrrw", IntReg.zero, IntReg.ra, CSR.PMPADDR0),
                ]

    # Zero out the performance monitor CSRs
    if not ("vexriscv" in test_params.design_name and is_forbid_vexriscv_csrs()):
        if test_params.design_name != "picorv32":
            instrs += [
                CSRRegInstr("csrrw", IntReg.zero, IntReg.zero, CSR.MCYCLE),
                CSRRegInstr("csrrw", IntReg.zero, IntReg.zero, CSR.MINSTRET),
                CSRRegInstr("csrrw", IntReg.zero, IntReg.zero, CSR.MCAUSE),
                CSRRegInstr("csrrw", IntReg.zero, IntReg.zero, CSR.MTVAL),
                CSRRegInstr("csrrw", IntReg.zero, IntReg.zero, CSR.MSCRATCH),
            ]
        if test_params.s_mode_support:
            instrs += [
                CSRRegInstr("csrrw", IntReg.zero, IntReg.zero, CSR.SCAUSE),
                CSRRegInstr("csrrw", IntReg.zero, IntReg.zero, CSR.STVAL),
                CSRRegInstr("csrrw", IntReg.zero, IntReg.zero, CSR.SSCRATCH),
            ]
        if not is_rv64 and test_params.design_name != "picorv32":
            instrs += [
                CSRRegInstr("csrrw", IntReg.zero, IntReg.zero, CSR.MCYCLEH),
                CSRRegInstr("csrrw", IntReg.zero, IntReg.zero, CSR.MINSTRETH),
            ]

    # Start with enabled FPU, if the FPU exists.
    if test_params.design_has_fpu:
        instrs += [
            # Enable the FPU
            CSRRegInstr("csrrw", IntReg.zero, FPU_STATE_BITS_REG[0], CSR.MSTATUS),
            # Set the initial rounding mode to zero initially for convineience
            CSRRegInstr("csrrw", IntReg.zero, IntReg.zero, CSR.FCSR),
        ]

    if test_params.s_mode_support or test_params.u_mode_support:
        instrs += [
            # Initialize MPP bits to M-mode for convinience
            CSRRegInstr("csrrs", IntReg.zero, MPP_BOTH_BITS_REG[0], CSR.MSTATUS),
        ]

    if not ("vexriscv" in test_params.design_name and is_forbid_vexriscv_csrs()):
        if test_params.u_mode_support:
            instrs += [
                RegImmInstr("addi", IntReg.ra, IntReg.zero, SPP_BIT_VAL, is_rv64),
                # Initialize SPP bits for convinience
                CSRRegInstr("csrrs", IntReg.zero, IntReg.ra, CSR.MSTATUS),
            ]

    # enable mstatus.mie, mie.msip
    if test_params.num_harts > 1:
        instrs += [
            CSRImmInstr("csrrsi", IntReg.zero, 0x8, CSR.MSTATUS),
            CSRImmInstr("csrrsi", IntReg.zero, 0x8, CSR.MIE),
        ]


def _connect_to_first_fuzzing_bb(
    instrs: "list[RVInstr]",
    is_rv64: bool,
    fuzzerstate: "FuzzerState",
    corestates: "dict[int, CoreState]",
):
    # Jump to the next basic block using a branch for simplicity
    branch_range_bits = get_range_bits(ISAInstrClass.BRANCH)
    instrs.append(FenceInstr("fence", FenceOrdering.RW_RW))
    instrs.append(CSRRegInstr("csrrs", MCM_RESET_REG, IntReg.zero, CSR.MHARTID))
    for hartid, core in corestates.items():
        assert hartid < 1023, "We support hart IDs up to 1023, add lui op for more"
        curr_addr = fuzzerstate.init_bb_base_addr + len(instrs) * ILEN
        core.next_bb_addr = fuzzerstate.memstate.gen_random_free_addr(
            BYTES_ALLIGN_4,
            BASIC_BLOCK_MIN_SPACE,
            0,
            curr_addr + (1 << branch_range_bits),
        )
        if core.next_bb_addr is None:
            return False
        fuzzerstate.memstate.alloc_mem_range(core.next_bb_addr, BASIC_BLOCK_MIN_SPACE)
        imm = core.next_bb_addr - curr_addr
        instrs.append(
            BranchInstr("beq", MCM_RESET_REG, IntReg.zero, imm, True, is_rv64)
        )
        instrs.append(RegImmInstr("addi", MCM_RESET_REG, MCM_RESET_REG, -1, is_rv64))
    # Only useful for MCM fuzzing, but kept for easier alignment prediction
    lui_imm, addi_imm = li_into_reg(STRESS_SECTION_BASE - RELOCATOR_REG[1])
    assert addi_imm == 0
    instrs += [
        ImmRdInstr("lui", MCM_RESET_REG, lui_imm, is_rv64),
        R12DInstr("add", MCM_RESET_REG, RELOCATOR_REG[0], MCM_RESET_REG),
        JALRInstr("jalr", IntReg.zero, MCM_RESET_REG, 0, None, is_rv64),
    ]


def gen_initial_basic_block(
    fuzzerstate: "FuzzerState",
    corestates: dict[int, "CoreState"],
    test_params: "TestParams",
) -> bool:
    """
    The first basic block is responsible for the initial setup. Each HART
    in the system will share this block. After initialization, we branch to the
    first fuzzing block of the current hart. This is the single first point
    where the execution path of HARTs diverge
    """
    is_rv64 = test_params.is_rv64
    instrs = []

    # Make the dedicated registers
    _gen_dedicated_regs(instrs, is_rv64, test_params.num_harts)

    # We assume that the whole memory is initialized to 0
    if test_params.num_harts > 1:
        _init_multicore(instrs, is_rv64, fuzzerstate)

    # Initialize various CSRs for consistency and enables FPU, interrupts and
    # other setup other features and states
    _initialize_csrs(instrs, is_rv64, test_params)

    # Different cores might have different sets of pickable regs, so all the
    # pickable regs are initialized for convinience
    # x0 does not need to be initialized
    all_pickable_intergs = list(
        {
            reg
            for core in corestates.values()
            for reg in core.intregpickstate.pickable_intregs
            if reg != IntReg.zero
        }
    )
    all_pickable_floatregs = list(
        {
            reg
            for core in corestates.values()
            for reg in core.floatregpickstate.pickable_floatregs
        }
    )
    if runparams.USE_ADDR_REGS:
        all_addr_regs = list(
            {reg for core in corestates.values() for reg in core.addr_regs}
        )
        all_addr_regs = sorted(list(all_addr_regs), key=lambda x: x.value)
    else:
        all_addr_regs = []
    fuzzerstate.all_addr_regs = all_addr_regs
    total_num_regs = (
        len(all_pickable_intergs) + len(all_pickable_floatregs) + len(all_addr_regs)
    )

    if __debug__:
        if not test_params.design_has_fpu:
            assert not all_pickable_floatregs
        if not runparams.USE_ADDR_REGS:
            assert not all_addr_regs
        assert fuzzerstate.num_store_locs >= 1

    # Initialize Registers that are not pickable
    intregs = list(IntReg)
    for reg in intregs:
        if (
            reg not in all_pickable_intergs
            and reg not in all_addr_regs
            and reg.value <= (MAX_NUM_PICKABLE_INTREGS)
        ):
            instrs.append(RegImmInstr("addi", reg, IntReg.zero, 0, is_rv64))
    floatregs = list(FloatReg)
    for freg in floatregs:
        if freg not in all_pickable_floatregs:
            instrs.append(FloatIntRs1Instr("fmv.w.x", freg, IntReg.zero, is_rv64))

    reginit_base_addr = (
        fuzzerstate.init_bb_base_addr
        + len(instrs) * ILEN  # previous instructions
        + total_num_regs * ILEN  # init instructions
        + 2 * ILEN  # load addr gen
        + (2 * test_params.num_harts + 2) * ILEN  # branches and hartid read
        + 3 * ILEN  # go to stress block
    )

    # add an allignemnt instruction so the datablock starts at an 8-byte
    # boundary
    if reginit_base_addr % 8 != 0:
        reginit_base_addr += ILEN
        assert reginit_base_addr % 8 == 0
        instrs.append(RegImmInstr("addi", IntReg.zero, IntReg.zero, 0x0, is_rv64))

    # Load the first address after the initial bb
    addr_reg = list(IntReg)[MAX_NUM_PICKABLE_INTREGS + 1]
    instrs.append(R12DInstr("add", addr_reg, IntReg.zero, RELOCATOR_REG[0]))
    instrs.append(RegImmInstr("addi", addr_reg, addr_reg, reginit_base_addr, is_rv64))

    # Floating loads must be done before int loads, because the register holding
    # The load address will be overwritten
    intload_instr = "ld" if is_rv64 else "lw"
    fload_instr = "fld" if is_rv64 else "flw"
    for freg_idx, frd in enumerate(all_pickable_floatregs):
        offset = 8 * freg_idx
        instrs.append(FloatLoadInstr(fload_instr, frd, addr_reg, offset, None, is_rv64))
    for reg_idx, rd in enumerate(all_pickable_intergs):
        offset = 8 * (reg_idx + len(all_pickable_floatregs))
        instrs.append(IntLoadInstr(intload_instr, rd, addr_reg, offset, None, is_rv64))
    # addr reg at the end, as the last intreg contains the address
    for reg_idx, rd in enumerate(all_addr_regs):
        offset = 8 * (reg_idx + len(all_pickable_floatregs) + len(all_pickable_intergs))
        instrs.append(IntLoadInstr(intload_instr, rd, addr_reg, offset, None, is_rv64))

    # generate the data for the initialization data
    for _ in range(total_num_regs):
        if test_params.prng.random() < test_params.proba_reg_starts_with_zero:
            fuzzerstate.initial_reg_data_content.append(0)
        else:
            fuzzerstate.initial_reg_data_content.append(
                test_params.prng.randrange(1 << 64)
            )

    # Allocate the initial block before choosing an address for the next bb.
    init_bb_instr_and_data = (
        (len(instrs) + 1) * ILEN
        + len(fuzzerstate.initial_reg_data_content) * DWORD_SIZE
        + (2 * test_params.num_harts + 2 + 3) * ILEN
    )  # NO_COMPRESSED
    fuzzerstate.memstate.alloc_mem_range(
        fuzzerstate.init_bb_base_addr,
        init_bb_instr_and_data,
    )

    # Connect the initial block to the first fuzzing basic block, this is the
    # divergance point for each hart
    _connect_to_first_fuzzing_bb(instrs, is_rv64, fuzzerstate, corestates)

    fuzzerstate.set_init_instrs(instrs)
    curr_addr = fuzzerstate.init_bb_base_addr + len(instrs) * ILEN
    fuzzerstate.initial_reg_data_addr = curr_addr

    if __debug__:
        assert curr_addr % 8 == 0, "Register init data block misaligned"
        assert curr_addr == reginit_base_addr, "Mispredicted first dword addr"

    return True
