# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


"""
Interrupt operation generation module.

This module handles generation of interrupt-related instruction sequences:
- Inter-Processor Interrupts (IPIs)
- Local software interrupts
- Wait sequences for trap and non-trap scenarios
- Interrupt clearing

Note: IPI and local interrupt generation have different flows:
  - IPI: Generate waiting sequence first, then send IPI from another core
  - Local: Send interrupt immediately, then wait
"""

from typing import TYPE_CHECKING, List
from enum import Enum, auto
from random import Random

from toleratebugs import is_tolerate_naxriscv_clint
from proggen.util import alloc_next_bb_addr
from riscv import PrivLvl, ILEN, MSIP_WIDTH, SPIKE_CLINT_BASE, IntReg, CSR
from params import CORESYNC_ADDR_REG, RDEP_MASK_REG, INTERRUPT_BARRIERS_OFFSET
from instgen import (
    ISAInstrClass,
    WfiInstr,
    InteruptInstr,
    RegImmInstr,
    IntStoreInstr,
    IntLoadInstr,
    BranchInstr,
    ImmRdInstr,
    JALInstr,
    TrapInstrWrapper,
    FenceInstr,
    FenceOrdering,
    CSRRegInstr,
    R12DInstr
)
from states import IntRegState, SyncState

if TYPE_CHECKING:
    from states import CoreState, FuzzerState
    from params import TestParams


class TrapWaitMethod(Enum):
    """Methods for waiting when an interrupt causes a trap."""

    WFI = auto()
    BUSY_LOOP = auto()
    STATE_PRESERVING_LOOP = auto()
    INTERRUPT_BLOCK = auto()


class NoTrapWaitMethod(Enum):
    """Methods for waiting when an interrupt does not cause a trap."""

    WFI = auto()
    SYNC_ONLY = auto()


# Constants
SYNC_FLAG_VALUE = 0xFF
LOOP_BACKWARD_OFFSET = -ILEN
DOUBLE_LOOP_BACKWARD_OFFSET = -2 * ILEN

# FIXME currently, the memeory consistency solver will account for interrupt
# related load and stores as well

# ============================================================================
# Public API Functions
# ============================================================================


def gen_waiting_instr_trap(
    fuzzerstate: "FuzzerState",
    corestate: "CoreState",
    test_params: "TestParams",
    sync: bool = True,
    num_local_int_instr: int = 0,
) -> List:
    """
    Generate instructions to wait for an interrupt that will cause a trap.

    Sets the hart in a low-power state and updates the tracked state.
    Before waiting, sets a flag in memory to synchronize with other cores.

    Args:
        fuzzerstate: Global fuzzer state
        corestate: Core-specific state
        test_params: Test configuration parameters
        sync: Whether to synchronize with other cores
        num_local_int_instr: Number of local interrupt instructions already generated

    Returns:
        List of generated instructions

    Note:
        Should only be called when the interrupt is expected to trap to M-mode.
    """
    instrs, wait_method = _generate_trap_wait_instructions(
        fuzzerstate, corestate, test_params, sync, num_local_int_instr
    )

    _update_interrupt_location(corestate, wait_method)

    if sync:
        fuzzerstate.waiting_core_ids.append(corestate.hartid)
        assert fuzzerstate.is_global_sync_state_free()
        fuzzerstate.sync_states[corestate.hartid] = SyncState.INTERRUPT

    _update_hart_state_for_trap(corestate, test_params, wait_method, fuzzerstate)

    return instrs


def gen_waiting_instr_notrap(
    fuzzerstate: "FuzzerState",
    corestate: "CoreState",
    test_params: "TestParams",
) -> List:
    """
    Generate instructions to wait for an interrupt that will NOT cause a trap.

    Synchronizes the hart to receive an interrupt that won't trap.

    Args:
        fuzzerstate: Global fuzzer state
        corestate: Core-specific state
        test_params: Test configuration parameters

    Returns:
        List of generated instructions
    """
    instrs = []

    # Set up synchronization if MIP not already set
    if not corestate.hartstate.mip_msip:
        instrs.extend(_generate_sync_instructions(corestate, test_params))
        corestate.hartstate.mip_msip = True
        fuzzerstate.waiting_core_ids.append(corestate.hartid)
        assert fuzzerstate.is_global_sync_state_free()
        fuzzerstate.sync_states[corestate.hartid] = SyncState.INTERRUPT

    # Generate wait instructions based on selected method
    wait_methods = _filter_notrap_wait_methods(corestate)
    wait_method = test_params.prng.choices(
        list(wait_methods.keys()), weights=list(wait_methods.values())
    )[0]

    instrs.extend(_generate_notrap_wait_sequence(corestate, test_params, wait_method))
    corestate.interrupt_locs.append((ISAInstrClass.WFI_NOTRAP, corestate.get_bb_idx()))

    return instrs


def send_local_interrupt(
    fuzzerstate: "FuzzerState",
    corestate: "CoreState",
    test_params: "TestParams",
) -> List:
    """
    Send a local software interrupt to self.

    This is similar to sending an IPI, but no cross-core synchronization is needed.

    Steps:
        1. Set up state loop register if interrupt will trap
        2. Send interrupt to self via CLINT
        3. Wait for interrupt (trap or non-trap path)

    Args:
        fuzzerstate: Global fuzzer state
        corestate: Core-specific state
        test_params: Test configuration parameters

    Returns:
        List of generated instructions
    """
    instrs = []
    hartstate = corestate.hartstate

    # Determine if interrupt will cause a trap
    will_trap = hartstate.mie_msie and (
        hartstate.privlvl != PrivLvl.Machine or hartstate.mstatus_mie
    )

    # Set up comparison register for trap case
    if will_trap:
        instrs.extend(_setup_state_loop_register(corestate, test_params))

    # Generate CLINT write instructions
    corestate.add_clint_instr(offset=len(instrs))
    instrs.extend(
        _generate_clint_write_instructions(
            corestate, test_params.is_rv64, corestate.hartid
        )
    )

    corestate.interrupt_locs.append(
        (ISAInstrClass.SEND_LOCAL_INTERRUPT, corestate.get_bb_idx())
    )

    # Wait for interrupt
    if will_trap:
        assert corestate.hartstate.is_mtvec_populated
        instrs.extend(
            gen_waiting_instr_trap(
                fuzzerstate, corestate, test_params, False, len(instrs)
            )
        )
    else:
        # Wait until interrupt appears in MIP
        hartstate.mip_msip = True
        rs1 = corestate.intregpickstate.pick_int_outputreg_nonzero()
        instrs.extend(
            [
                CSRRegInstr("csrrs", rs1, IntReg.zero, CSR.MIP),
                BranchInstr(
                    "beq",
                    rs1,
                    IntReg.zero,
                    LOOP_BACKWARD_OFFSET,
                    True,
                    test_params.is_rv64,
                    fixed_opcode=True,
                ),
            ]
        )

    return instrs


def send_ipi(
    fuzzerstate: "FuzzerState",
    corestate: "CoreState",
    is_rv64: bool,
    prng: Random,
) -> List:
    """
    Send an Inter-Processor Interrupt (IPI) to another core.

    Sends an IPI by writing to the MSIP register of the target core's CLINT.

    Steps:
        1. Wait for target core to signal it's ready (sync address)
        2. Reset synchronization address
        3. Send interrupt via CLINT write

    Args:
        fuzzerstate: Global fuzzer state
        corestate: Core-specific state
        is_rv64: Whether this is a 64-bit RISC-V system
        prng: Random number generator for selecting target

    Returns:
        List of generated instructions
    """
    # Select target core from waiting cores
    target_core_id = prng.choice(
        [
            core_id
            for core_id in fuzzerstate.waiting_core_ids
            if core_id != corestate.hartid
        ]
    )
    fuzzerstate.waiting_core_ids.remove(target_core_id)
    assert not fuzzerstate.is_global_sync_state_free()
    fuzzerstate.sync_states[target_core_id] = SyncState.FREE

    # Allocate registers
    rs2 = corestate.intregpickstate.pick_int_outputreg_nonzero()
    # Synchronize
    barrier_offset = INTERRUPT_BARRIERS_OFFSET + target_core_id
    instrs = [
        # Wait for target core to be ready (spinning on sync address)
        IntLoadInstr(
            "lbu",
            rs2,
            CORESYNC_ADDR_REG[0],
            barrier_offset,
            None,
            is_rv64,
            is_interrupt=True,
            dest_addr=CORESYNC_ADDR_REG[1] + barrier_offset,
        ),
        RegImmInstr("addi", rs2, rs2, -SYNC_FLAG_VALUE, is_rv64),
        BranchInstr(
            "bne",
            rs2,
            IntReg.zero,
            DOUBLE_LOOP_BACKWARD_OFFSET,
            True,
            is_rv64,
            fixed_opcode=True,
        ),
        # Prepare interrupt value
    ]

    # Write to CLINT
    corestate.add_clint_instr(offset=len(instrs))
    instrs += _generate_clint_write_instructions(corestate, is_rv64, target_core_id)

    # clear synchronization
    barrier_offset = INTERRUPT_BARRIERS_OFFSET + target_core_id
    instrs += [
        IntStoreInstr(
            "sb",
            CORESYNC_ADDR_REG[0],
            IntReg.zero,
            barrier_offset,
            None,
            is_rv64,
            is_interrupt=True,
            dest_addr=CORESYNC_ADDR_REG[1] + barrier_offset,
        ),
        FenceInstr("fence", FenceOrdering.RW_RW),
    ]

    corestate.interrupt_locs.append((ISAInstrClass.SEND_IPI, corestate.get_bb_idx()))

    return instrs


def clear_pending_interrupt(corestate: "CoreState", is_rv64: bool) -> List:
    """
    Clear a pending software interrupt in the core's MSIP register.

    Args:
        corestate: Core-specific state
        is_rv64: Whether this is a 64-bit RISC-V system

    Returns:
        List of generated instructions
    """
    corestate.hartstate.mip_msip = False

    rs1 = corestate.intregpickstate.pick_int_outputreg_nonzero()
    corestate.intregpickstate.set_regstate(rs1, IntRegState.POLLUTED)

    lui_imm = SPIKE_CLINT_BASE >> 12
    msip_offset = corestate.hartid * MSIP_WIDTH

    store_instr = "sw"
    if is_tolerate_naxriscv_clint():
        store_instr = "sb"

    corestate.add_clint_instr()
    instrs = [
        ImmRdInstr("lui", rs1, lui_imm, is_rv64),
        R12DInstr("and", rs1, rs1, RDEP_MASK_REG[0]),
        IntStoreInstr(
            store_instr,
            rs1,
            IntReg.zero,
            msip_offset,
            None,
            is_rv64,
            is_interrupt=True,
            dest_addr=SPIKE_CLINT_BASE + msip_offset,
        ),
        IntLoadInstr(
            "lb",
            rs1,
            rs1,
            msip_offset,
            None,
            is_rv64,
            is_interrupt=True,
            dest_addr=SPIKE_CLINT_BASE + msip_offset,
        ),
        BranchInstr(
            "bne",
            rs1,
            IntReg.zero,
            DOUBLE_LOOP_BACKWARD_OFFSET,
            True,
            is_rv64,
            fixed_opcode=True,
        ),
    ]

    corestate.interrupt_locs.append(
        (ISAInstrClass.CLEAR_INTERRUPT, corestate.get_bb_idx())
    )

    return instrs


# ============================================================================
# Private Helper Functions - State Management
# ============================================================================


def _update_interrupt_location(
    corestate: "CoreState", wait_method: TrapWaitMethod
) -> None:
    """Update the interrupt location tracking based on wait method."""
    bb_offset = 1 if wait_method == TrapWaitMethod.STATE_PRESERVING_LOOP else 0
    corestate.interrupt_locs.append(
        (ISAInstrClass.WFI_TRAP, corestate.get_bb_idx() + bb_offset)
    )


def _update_hart_state_for_trap(
    corestate: "CoreState",
    test_params: "TestParams",
    wait_method: TrapWaitMethod,
    fuzzerstate: "FuzzerState",
) -> None:
    """Update hart state after a trap caused by an interrupt."""
    hartstate = corestate.hartstate

    # Common state updates
    hartstate.is_mtvec_populated = False
    hartstate.is_mepc_populated = False
    hartstate.mstatus_mpp = hartstate.privlvl
    hartstate.mip_msip = True
    hartstate.mstatus_mpie = hartstate.mstatus_mie
    hartstate.mstatus_mie = False
    # we can trap before executing the wait instruction
    hartstate.is_mepc_non_deterministic = True

    # Update privilege level and statistics based on wait method
    if wait_method in (
        TrapWaitMethod.STATE_PRESERVING_LOOP,
        TrapWaitMethod.INTERRUPT_BLOCK,
    ):
        fuzzerstate.landing_type["hard"] += 1
    else:
        hartstate.privlvl = PrivLvl.Machine
        fuzzerstate.landing_type["soft"] += 1

    # Handle double trap extension
    if "smdbltrp" in test_params.design_extentions:
        hartstate.mstatus_mdt = True


def _setup_state_loop_register(
    corestate: "CoreState",
    test_params: "TestParams",
) -> List:
    """Set up register for state-preserving loop or interrupt block."""
    corestate.stateloop_cmp_reg = corestate.intregpickstate.pick_int_outputreg_nonzero()
    corestate.intregpickstate.set_regstate(
        corestate.stateloop_cmp_reg, IntRegState.RESERVED
    )
    return [
        RegImmInstr(
            "addi",
            corestate.stateloop_cmp_reg,
            IntReg.zero,
            0,
            test_params.is_rv64,
        )
    ]


# ============================================================================
# Private Helper Functions - Instruction Generation
# ============================================================================


def _generate_sync_instructions(
    corestate: "CoreState",
    test_params: "TestParams",
) -> List:
    """Generate synchronization instructions."""
    barrier_offset = INTERRUPT_BARRIERS_OFFSET + corestate.hartid
    return [
        IntStoreInstr(
            "sb",
            CORESYNC_ADDR_REG[0],
            RDEP_MASK_REG[0],
            barrier_offset,
            None,
            test_params.is_rv64,
            is_interrupt=True,
            dest_addr=CORESYNC_ADDR_REG[1] + barrier_offset,
        ),
        FenceInstr("fence", FenceOrdering.RW_RW),
    ]


def _generate_notrap_wait_sequence(
    corestate: "CoreState",
    test_params: "TestParams",
    wait_method: NoTrapWaitMethod,
) -> List:
    """Generate instruction sequence for waiting without trap."""
    rd = corestate.intregpickstate.pick_int_outputreg_nonzero()
    barrier_offset = INTERRUPT_BARRIERS_OFFSET + corestate.hartid
    if wait_method == NoTrapWaitMethod.WFI:
        # WFI can be implemented as NOP, so we must synchronize to ensure
        # we don't reach the next sync point before the other core
        return [
            WfiInstr(),
            IntLoadInstr(
                "lbu",
                rd,
                CORESYNC_ADDR_REG[0],
                barrier_offset,
                None,
                test_params.is_rv64,
                is_interrupt=True,
                dest_addr=CORESYNC_ADDR_REG[1] + barrier_offset,
            ),
            BranchInstr(
                "bne",
                rd,
                IntReg.zero,
                LOOP_BACKWARD_OFFSET,
                True,
                test_params.is_rv64,
                fixed_opcode=True,
            ),
        ]
    elif wait_method == NoTrapWaitMethod.SYNC_ONLY:
        return [
            IntLoadInstr(
                "lbu",
                rd,
                CORESYNC_ADDR_REG[0],
                barrier_offset,
                None,
                test_params.is_rv64,
                is_interrupt=True,
                dest_addr=CORESYNC_ADDR_REG[1] + barrier_offset,
            ),
            BranchInstr(
                "bne",
                rd,
                IntReg.zero,
                LOOP_BACKWARD_OFFSET,
                True,
                test_params.is_rv64,
                fixed_opcode=True,
            ),
        ]
    else:
        raise ValueError(f"Unknown no-trap wait method: {wait_method}")


def _generate_clint_write_instructions(
    corestate: "CoreState",
    is_rv64: bool,
    target_hartid: int,
) -> List:
    """
    Generate instructions to write to CLINT MSIP register.

    Args:
        corestate: Core-specific state
        test_params: Test configuration parameters
        target_hartid: Target hart ID for the interrupt

    Returns:
        List of CLINT write instructions
    """
    rs1 = corestate.intregpickstate.pick_int_outputreg_nonzero()
    corestate.intregpickstate.set_regstate(rs1, IntRegState.RESERVED)
    rs2 = corestate.intregpickstate.pick_int_outputreg_nonzero()
    # Mark as polluted because spike and design CLINT addresses can differ
    corestate.intregpickstate.set_regstate(rs1, IntRegState.POLLUTED, force=True)

    lui_imm = SPIKE_CLINT_BASE >> 12
    store_instr = "sw"
    if is_tolerate_naxriscv_clint():
        store_instr = "sb"

    return [
        ImmRdInstr("lui", rs1, lui_imm, is_rv64),
        R12DInstr("and", rs1, rs1, RDEP_MASK_REG[0]),
        RegImmInstr("ori", rs2, IntReg.zero, 1, is_rv64),
        InteruptInstr(
            store_instr,
            rs1,
            rs2,
            target_hartid * MSIP_WIDTH,
            None,
            target_hartid,
            is_rv64,
        ),
    ]


# ============================================================================
# Private Helper Functions - Trap Wait Instructions
# ============================================================================


def _generate_trap_wait_instructions(
    fuzzerstate: "FuzzerState",
    corestate: "CoreState",
    test_params: "TestParams",
    sync_signal: bool,
    num_local_int_instr: int,
) -> tuple[List, TrapWaitMethod]:
    """
    Generate instruction sequence for waiting when interrupt will cause trap.

    Returns:
        Tuple of (instruction list, wait method used)
    """
    wait_methods = _filter_trap_wait_methods(corestate, test_params)
    wait_method = test_params.prng.choices(
        list(wait_methods.keys()), weights=list(wait_methods.values())
    )[0]

    # Generate sync instruction or NOP for local interrupts
    barrier_offset = INTERRUPT_BARRIERS_OFFSET + corestate.hartid
    if sync_signal:
        trap_sync_instr = IntStoreInstr(
            "sb",
            CORESYNC_ADDR_REG[0],
            RDEP_MASK_REG[0],
            barrier_offset,
            None,
            test_params.is_rv64,
            is_interrupt=True,
            dest_addr=CORESYNC_ADDR_REG[1] + barrier_offset,
        )
    else:
        trap_sync_instr = RegImmInstr(
            "addi", IntReg.zero, IntReg.zero, 0, test_params.is_rv64
        )

    # Base instructions
    instrs = [
        TrapInstrWrapper(
            is_mtvec=True,
            producer_id=None,
            instr=trap_sync_instr,
            prev_priv=corestate.hartstate.privlvl,
        ),
        FenceInstr("fence", FenceOrdering.RW_RW),
    ]

    # Mark special interrupt BB if needed
    if wait_method in (
        TrapWaitMethod.STATE_PRESERVING_LOOP,
        TrapWaitMethod.INTERRUPT_BLOCK,
    ):
        assert isinstance(instrs[0], TrapInstrWrapper)
        instrs[0].special_interrupt_bb = True

        # Set up state loop register if syncing
        if sync_signal:
            instrs = _setup_state_loop_register(corestate, test_params) + instrs

    # Add wait method-specific instructions
    wait_instrs = _generate_wait_method_instructions(
        fuzzerstate,
        corestate,
        test_params,
        wait_method,
        sync_signal,
        num_local_int_instr,
        instrs,
    )
    instrs.extend(wait_instrs)

    return instrs, wait_method


def _generate_wait_method_instructions(
    fuzzerstate: "FuzzerState",
    corestate: "CoreState",
    test_params: "TestParams",
    wait_method: TrapWaitMethod,
    sync_signal: bool,
    num_local_int_instr: int,
    base_instrs: List,
) -> List:
    """Generate instructions specific to the chosen wait method."""
    if wait_method == TrapWaitMethod.WFI:
        return [
            WfiInstr(),
            JALInstr("jal", IntReg.zero, LOOP_BACKWARD_OFFSET, is_absolute=True),
        ]

    elif wait_method == TrapWaitMethod.BUSY_LOOP:
        return [JALInstr("jal", IntReg.zero, 0, is_absolute=True)]

    elif wait_method == TrapWaitMethod.STATE_PRESERVING_LOOP:
        # Set up state-preserving loop
        corestate.state_preserving_loop = True
        corestate.interrupt_handlers_start_addr.append(corestate.get_next_bb_addr())
        corestate.next_bb_addr = None

        # Generate jump to loop
        curr_addr = (
            corestate.get_current_addr()
            + (len(base_instrs) + num_local_int_instr) * ILEN
        )
        next_addr_gen_success = alloc_next_bb_addr(
            fuzzerstate, corestate, ISAInstrClass.JAL, curr_addr
        )

        if not next_addr_gen_success:
            raise RuntimeError("Unexpectedly ran out of address space")

        imm = corestate.get_next_bb_addr() - curr_addr
        return [JALInstr("jal", IntReg.zero, imm, False)]

    elif wait_method == TrapWaitMethod.INTERRUPT_BLOCK:
        # Set up interrupt block
        corestate.interrupt_block = True
        corestate.interrupt_handlers_start_addr.append(corestate.get_next_bb_addr())
        corestate.next_bb_addr = None

        # Generate hard jump to next basic block
        curr_addr = (
            corestate.get_current_addr()
            + (len(base_instrs) + num_local_int_instr) * ILEN
        )
        next_addr_gen_success = alloc_next_bb_addr(
            fuzzerstate, corestate, ISAInstrClass.JAL, curr_addr
        )

        if not next_addr_gen_success:
            raise RuntimeError("Unexpectedly ran out of address space")

        imm = corestate.get_next_bb_addr() - curr_addr
        return [JALInstr("jal", IntReg.zero, imm)]

    else:
        raise ValueError(f"Unknown trap wait method: {wait_method}")


# ============================================================================
# Private Helper Functions - Wait Method Filtering
# ============================================================================


def _filter_notrap_wait_methods(corestate: "CoreState") -> dict:
    """Filter available wait methods for non-trapping interrupts."""
    wait_methods = {op: 1 for op in NoTrapWaitMethod}

    if not corestate.hartstate.mie_msie:
        # WFI will never unblock if MIE_MSIE is not set
        wait_methods[NoTrapWaitMethod.WFI] = 0

    if corestate.hartstate.privlvl == PrivLvl.User:
        wait_methods[NoTrapWaitMethod.WFI] = 0

    return wait_methods


def _filter_trap_wait_methods(
    corestate: "CoreState",
    test_params: "TestParams",
) -> dict:
    """Filter available wait methods for trapping interrupts."""
    wait_methods = {op: 1 for op in TrapWaitMethod}

    is_wfi_allowed_for_user = (
        corestate.hartstate.privlvl == PrivLvl.User and test_params.wfi_u_mode_avail
    )

    if not is_wfi_allowed_for_user:
        wait_methods[TrapWaitMethod.WFI] = 0

    # State-preserving loop and interrupt block require consumed registers
    # when not in machine mode
    if (
        corestate.hartstate.privlvl != PrivLvl.Machine
        and not corestate.intregpickstate.exists_reg_in_state(IntRegState.CONSUMED)
    ):
        wait_methods[TrapWaitMethod.STATE_PRESERVING_LOOP] = 0
        wait_methods[TrapWaitMethod.INTERRUPT_BLOCK] = 0

    return wait_methods
