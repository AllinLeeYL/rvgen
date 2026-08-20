# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


from random import Random
from typing import TYPE_CHECKING
from proggen.util import (
    alloc_next_bb_addr,
    alloc_next_loopsegment,
    create_instr_stateloop,
    create_instr,
)
from params import (
    BASIC_BLOCK_MIN_SPACE,
    MPIE_BIT_VAL,
    MAX_STATE_PRESERVING_LOOP_INSTR,
    BRANCH_TAKEN_PROBA,
    ISAINSTRCLASS_INITIAL_BOOSTERS,
)

# Maximum branch range for RISC-V branch instructions (12-bit signed immediate)
# The immediate represents offsets in multiples of 2 bytes, giving ±4096 bytes range
MAX_BRANCH_RANGE = 1 << 12  # 4096 bytes
from instgen import (
    INSTRS_BY_ISA_CLASS,
    PrivDescentInstr,
    BranchInstr,
    RegImmInstr,
    CSRRegInstr,
    CSRImmInstr,
    TvecWriterInstr,
    ISAInstrClass,
    RVInstr,
)
from riscv import PrivLvl, ILEN, IntReg, CSR, FpuState, IntReg, FloatReg
from states import IntRegState, FloatRegState

if TYPE_CHECKING:
    from states import CoreState, FuzzerState
    from params import TestParams


def gen_minimal_interrupt_handler(
    corestate: "CoreState", is_rv64: bool, has_smdbltrp_ext: bool
):
    """Appends a minimal interrupt handling routine to the list of basic blocks.
    The routine disables MPIE to avois trapping upon return, and returns
    """
    instrs = []

    # If the priviledge state prior to the trap is not Machine mode, we need to
    # add a mtvec writer instruction in the handler (in M-mode), or we will
    # not be able to leave M-mode after the trap is taken
    rs1 = None
    if corestate.hartstate.privlvl != PrivLvl.Machine:
        assert corestate.intregpickstate.exists_reg_in_state(IntRegState.CONSUMED)
        rs1 = corestate.intregpickstate.pick_reg_in_state(IntRegState.CONSUMED)
        producer_id = corestate.intregpickstate.get_producer_id(rs1)
        # Ensure stateloop or interrupt block do not use it
        corestate.intregpickstate.set_regstate(rs1, IntRegState.RESERVED, force=True)
        instrs.append(
            TvecWriterInstr(
                is_mtvec=True, rd=IntReg.zero, rs1=rs1, producer_id=producer_id
            )
        )
        corestate.hartstate.is_mtvec_populated = True

    # Update tracking,
    # interrput set mipe=mie, mret sets mie=mpie, so no update to mie
    # same for mpp and priv level
    if has_smdbltrp_ext:
        corestate.hartstate.mstatus_mdt = False
    corestate.hartstate.mstatus_mpie = True
    corestate.hartstate.mstatus_mpp = PrivLvl.User
    rd = corestate.stateloop_cmp_reg
    assert rd

    instrs.append(RegImmInstr("addi", rd, IntReg.zero, MPIE_BIT_VAL, is_rv64))

    if corestate.hartstate.privlvl == PrivLvl.Machine:
        instrs.append(CSRRegInstr("csrrc", IntReg.zero, rd, CSR.MSTATUS))
    else:
        corestate.hartstate.mie_msie = False
        instrs.append(CSRImmInstr("csrrci", rd, 0x8, CSR.MIE))
    instrs.append(CSRRegInstr("csrrs", rd, IntReg.zero, CSR.MIP))
    instrs.append(
        PrivDescentInstr(
            is_mret=True,
            prev_priv=PrivLvl.Machine,
            is_helper=True,
        )
    )
    corestate.add_interrupt_handler(instrs)
    assert ILEN * len(instrs) <= BASIC_BLOCK_MIN_SPACE
    return rs1


def gen_state_preserving_loop(
    fuzzerstate: "FuzzerState",
    corestate: "CoreState",
    test_params: "TestParams",
) -> bool:
    """Generates a state preserving loop. There are 3 main parts:
    1. Initialize some registers
    2. Generate some instructions, using only initialized regs.
    3. Loop back as long as no interrupt is pending

    Returns:
        bool: True if loop generation succeeded, False otherwise
    """
    cmp_reg = corestate.stateloop_cmp_reg
    assert cmp_reg

    # Pick allowed instructions for the loop
    allowed_instrs = _pick_allowed_instr(corestate, test_params)

    # Generate the instructions in the loop
    loop_start_addr = corestate.get_current_addr()
    max_instr_count = test_params.prng.randint(1, MAX_STATE_PRESERVING_LOOP_INSTR)

    # Generate loop body
    if not _generate_loop_body(
        fuzzerstate,
        corestate,
        test_params,
        allowed_instrs,
        loop_start_addr,
        max_instr_count,
    ):
        return False

    # Generate loop exit (branch back and exit jump)
    return _generate_loop_exit(
        fuzzerstate, corestate, test_params, loop_start_addr, cmp_reg
    )


def _pick_allowed_instr(corestate: "CoreState", test_params: "TestParams"):
    """picks the intructions allowed in the state preserving loop
    We only allow instruction that can regain their value, so they
    cannot depend on a control state of the CPU
    """
    allowed_instrs: list[str] = []
    allowed_instrs += INSTRS_BY_ISA_CLASS[ISAInstrClass.ALU]
    allowed_instrs += INSTRS_BY_ISA_CLASS[ISAInstrClass.MULDIV]
    allowed_instrs += INSTRS_BY_ISA_CLASS[ISAInstrClass.JAL]
    allowed_instrs += INSTRS_BY_ISA_CLASS[ISAInstrClass.BRANCH]
    if test_params.is_rv64:
        allowed_instrs += INSTRS_BY_ISA_CLASS[ISAInstrClass.ALU64]
        allowed_instrs += INSTRS_BY_ISA_CLASS[ISAInstrClass.MULDIV64]

    # fixme make this an argument
    if ISAINSTRCLASS_INITIAL_BOOSTERS[ISAInstrClass.FPU] != 0:
        is_fpu_activated = corestate.hartstate.mstatus_fs != FpuState.Off
        if test_params.design_has_fpu and is_fpu_activated:
            allowed_instrs += INSTRS_BY_ISA_CLASS[ISAInstrClass.FPU]
        if test_params.design_has_fpud and is_fpu_activated:
            allowed_instrs += INSTRS_BY_ISA_CLASS[ISAInstrClass.FPUD]

        if test_params.is_rv64:
            if test_params.design_has_fpu and is_fpu_activated:
                allowed_instrs += INSTRS_BY_ISA_CLASS[ISAInstrClass.FPU64]
            if test_params.design_has_fpud and is_fpu_activated:
                allowed_instrs += INSTRS_BY_ISA_CLASS[ISAInstrClass.FPUD64]
    return allowed_instrs


def _register_new_regstates(corestate: "CoreState", instr: RVInstr):
    """Updates the state of register to preserve the state preservation property
    of the loop. We follow to following rules:
    1. A register used as an output is INITIALIZED, and can be used again as an
        input or an output in subsequent instructions, because its value will
        be re-initialized each iteration
    2. A register used as an input, and not yet initialized becomes restricted,
        or its values will chage between iterations as its value cannot be
        recomputed
    """

    # output registers not yet initialized and initialized
    outputregs = instr.get_outputregs()
    for outputreg in outputregs:
        if isinstance(outputreg, IntReg):
            regstate = corestate.intregpickstate
            is_initialized = regstate.get_regstate(outputreg) == IntRegState.INITIALIZED
            if not is_initialized:
                regstate.set_regstate(outputreg, IntRegState.INITIALIZED)
        elif isinstance(outputreg, FloatReg):
            regstate = corestate.floatregpickstate
            is_initialized = (
                regstate.get_regstate(outputreg) == FloatRegState.INITIALIZED
            )
            if not is_initialized:
                regstate.set_regstate(outputreg, FloatRegState.INITIALIZED)
        else:
            # CSR side effects
            continue

    # input registers not yet restricted, or not initialized become restricted
    inputregs = instr.get_inputregs()
    for initialized in inputregs:
        if isinstance(initialized, IntReg):
            regstate = corestate.intregpickstate
            is_restricted = regstate.get_regstate(initialized) == IntRegState.RESTRICTED
            is_initialized = (
                regstate.get_regstate(initialized) == IntRegState.INITIALIZED
            )
            if not (is_restricted or is_initialized):
                regstate.set_regstate(initialized, IntRegState.RESTRICTED)
        elif isinstance(initialized, FloatReg):
            regstate = corestate.floatregpickstate
            is_restricted = (
                regstate.get_regstate(initialized) == FloatRegState.RESTRICTED
            )
            is_initialized = (
                regstate.get_regstate(initialized) == FloatRegState.INITIALIZED
            )
            if not (is_restricted or is_initialized):
                regstate.set_regstate(initialized, FloatRegState.RESTRICTED)
        else:
            # CSR side effects
            continue


def _is_branch_or_jal_instr(instr_str: str) -> tuple[bool, bool]:
    """Check if instruction is a branch or JAL instruction.

    Returns:
        tuple: (is_jal, is_branch)
    """
    is_jal = instr_str in INSTRS_BY_ISA_CLASS[ISAInstrClass.JAL]
    is_branch = instr_str in INSTRS_BY_ISA_CLASS[ISAInstrClass.BRANCH]
    return is_jal, is_branch


def _should_allocate_next_segment(instr_str: str, corestate: "CoreState") -> bool:
    """Determine if we need to allocate a new segment for this instruction."""
    is_jal, is_branch = _is_branch_or_jal_instr(instr_str)
    return is_jal or (is_branch and corestate.curr_branch_taken)


def _generate_loop_instruction(
    fuzzerstate: "FuzzerState",
    corestate: "CoreState",
    test_params: "TestParams",
    allowed_instrs: list[str],
    loop_start_addr: int,
    max_instr_count: int,
) -> tuple[bool, bool]:
    """Generate a single instruction in the state preserving loop.

    Returns:
        tuple: (success, next_segment_allocated)
    """
    assert corestate.floatregpickstate._in_stateloop == True

    curr_addr = corestate.get_current_addr()
    instr_str = test_params.prng.choice(allowed_instrs)

    # Determine if branch should be taken
    _, is_branch = _is_branch_or_jal_instr(instr_str)
    if is_branch:
        corestate.curr_branch_taken = test_params.prng.random() < BRANCH_TAKEN_PROBA

    # Allocate new segment if needed (for JAL or taken branches)
    next_segment_allocated = False
    if _should_allocate_next_segment(instr_str, corestate):
        alloc_size = ILEN * max(BASIC_BLOCK_MIN_SPACE, max_instr_count)
        next_segment_allocated = alloc_next_loopsegment(
            fuzzerstate,
            corestate,
            ISAInstrClass.BRANCH,
            corestate.get_current_addr(),
            loop_start_addr,
            alloc_size,
        )
        if not next_segment_allocated:
            return False, False

    # Create and register the instruction
    instr = create_instr_stateloop(
        instr_str,
        corestate,
        fuzzerstate,
        test_params.prng,
        curr_addr,
        test_params.is_rv64,
    )
    assert len(instr) == 1

    _register_new_regstates(corestate, instr[0])
    corestate.add_instr(instr)

    # Initialize new basic block if needed
    if _should_allocate_next_segment(instr_str, corestate):
        corestate.init_new_bb()

    return True, next_segment_allocated


def _generate_loop_body(
    fuzzerstate: "FuzzerState",
    corestate: "CoreState",
    test_params: "TestParams",
    allowed_instrs: list[str],
    loop_start_addr: int,
    max_instr_count: int,
) -> bool:
    """Generate all instructions in the loop body.

    Returns:
        bool: True if successful, False otherwise
    """
    curr_alloc_cursor = corestate.get_curr_bb_start_addr() + BASIC_BLOCK_MIN_SPACE
    current_instr_count = 0

    # FIXME: make sure there is enough space before we reach the end of mem
    while (
        fuzzerstate.memstate.get_available_contig_space(curr_alloc_cursor) > ILEN
        and current_instr_count < max_instr_count
    ):
        # Check if we're within branch range to loop back
        # We need to account for the branch instruction itself (ILEN bytes)
        current_addr = corestate.get_current_addr()
        loop_distance = abs(current_addr - loop_start_addr)

        # Reserve space for the branch-back instruction and ensure we stay within range
        if loop_distance + ILEN > MAX_BRANCH_RANGE:
            # Stop generating instructions if we're at the branch range limit
            break

        success, next_segment_allocated = _generate_loop_instruction(
            fuzzerstate,
            corestate,
            test_params,
            allowed_instrs,
            loop_start_addr,
            max_instr_count,
        )

        if not success:
            return False

        # Update allocation cursor
        if not next_segment_allocated:
            fuzzerstate.memstate.alloc_mem_range(curr_alloc_cursor, ILEN)
            curr_alloc_cursor += ILEN
        else:
            curr_alloc_cursor = corestate.get_current_addr() + BASIC_BLOCK_MIN_SPACE

        current_instr_count += 1

    return True


def _generate_loop_exit(
    fuzzerstate: "FuzzerState",
    corestate: "CoreState",
    test_params: "TestParams",
    loop_start_addr: int,
    cmp_reg: IntReg,
) -> bool:
    """Generate the loop exit instructions (branch back and exit jump).

    Returns:
        bool: True if successful, False otherwise
    """
    is_rv64 = test_params.is_rv64
    instrs: list = []
    distance_to_start = loop_start_addr - corestate.get_current_addr()

    assert not fuzzerstate.memstate.is_mem_free(corestate.get_current_addr())

    # Branch back to loop start
    instrs.append(
        BranchInstr(
            "beq",
            cmp_reg,
            IntReg.zero,
            distance_to_start,
            True,
            is_rv64,
            fixed_opcode=True,
        )
    )

    # Exit jump
    curr_addr = corestate.get_current_addr() + len(instrs) * ILEN
    success = alloc_next_bb_addr(fuzzerstate, corestate, ISAInstrClass.JAL, curr_addr)
    if not success:
        return False

    instrs.extend(
        create_instr(
            "jal",
            corestate,
            fuzzerstate,
            test_params.prng,
            curr_addr,
            test_params.is_rv64,
        )
    )

    corestate.add_instr(instrs)
    return True
