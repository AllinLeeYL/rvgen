# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


from random import Random
from typing import TYPE_CHECKING
from proggen.util.createinst import create_instr
from proggen.blockgen import (
    gen_basicblock,
    gen_state_preserving_loop,
    gen_minimal_interrupt_handler,
)
from proggen.util.pickmemop import pick_memop_addr
from proggen.instrgen import gen_exception_from_cause
from instgen import (
    INSTRS_BY_ISA_CLASS,
    is_placeholder,
    TrapInstr,
    PrivDescentInstr,
    JALRInstr,
    JALInstr,
    BranchInstr,
    MisalignedMemInstr,
    IntLoadInstr,
    IntStoreInstr,
    FloatLoadInstr,
    FloatStoreInstr,
    AmoInstr,
    AmoStore,
    PlaceholderProducerInstr0,
    PlaceholderProducerInstr1,
    PlaceholderConsumerInstr,
    get_range_bits,
    ISAInstrClass,
)
from riscv import (
    PrivLvl,
    IntReg,
    ILEN,
)
import runparams
from states import IntRegState

if TYPE_CHECKING:
    from states import CoreState, FuzzerState
    from mcm import MCMState
    from params import TestParams


# Blacklist addresses where instructions change between spike resolution and RTL sim.
def blacklist_changing_instructions(
    fuzzerstate: "FuzzerState", corestates: list["CoreState"]
):
    """Blacklist addresses where instructions change between spike resolution and
    RTL sim. This includes the final block, the context setter block and some
    instructions like branches and placeholders.

    Without it, a load might try to load those operations, which will result in
    different values between RTL and pre-isa simulations
    """
    # All instructions whose bytecode depends on the is_spike_resolution boolean
    changing_instrs_types = (
        BranchInstr,
        PlaceholderProducerInstr0,
        PlaceholderProducerInstr1,
        PlaceholderConsumerInstr,
    )

    fuzzerstate.memview_blacklist.alloc_mem_range(
        fuzzerstate.init_bb_base_addr, ILEN * len(fuzzerstate.init_bb_instrs)
    )  # NO_COMPRESSED

    for core in corestates:
        for bb_id, bb_instrlist in enumerate(core.basic_blocks):
            changing_instrs = [
                instr_id
                for instr_id, instr in enumerate(bb_instrlist)
                if isinstance(instr, changing_instrs_types)
            ]
            for instr_id in changing_instrs:
                curr_addr = core.bb_start_addrs[bb_id] + instr_id * ILEN
                fuzzerstate.memview_blacklist.alloc_mem_range(curr_addr, ILEN)

    fuzzerstate.memview_blacklist.alloc_mem_range(
        fuzzerstate.get_final_bb_base_addr(),
        len(fuzzerstate.final_bb_instrs) * ILEN,
    )  # NO_COMPRESSED

    # Blacklist CLINT instructions
    for core in corestates:
        for bb_id, instr_id in core.clint_instr:
            assert len(core.bb_start_addrs) > bb_id, (
                f"{len(core.bb_start_addrs)} addrs, access: {bb_id} ({len(core.basic_blocks)} bbs)"
            )
            curr_addr = core.bb_start_addrs[bb_id] + instr_id * ILEN
            fuzzerstate.memview_blacklist.alloc_mem_range(curr_addr, ILEN)


def gen_stateloop(
    corestate: "CoreState", fuzzerstate: "FuzzerState", test_params: "TestParams"
):
    """Generates a state preserving loop block"""
    has_smdbltrp_ext = "smdbltrp" in test_params.design_extentions
    mtvec_prod = gen_minimal_interrupt_handler(
        corestate, test_params.is_rv64, has_smdbltrp_ext
    )
    corestate.enter_stateloop_gen()
    is_loopgen_success = gen_state_preserving_loop(fuzzerstate, corestate, test_params)
    corestate.exit_stateloop_gen()
    if not is_loopgen_success:
        return False
    if mtvec_prod is not None:
        corestate.intregpickstate.set_regstate(mtvec_prod, IntRegState.FREE)
    return True


def gen_interrupt_block(
    corestate: "CoreState", fuzzerstate: "FuzzerState", test_params: "TestParams"
):
    """Generates a block which blocks until an interrupt is recieved"""
    assert corestate.stateloop_cmp_reg
    has_smdbltrp_ext = "smdbltrp" in test_params.design_extentions
    mtvec_prod = gen_minimal_interrupt_handler(
        corestate, test_params.is_rv64, has_smdbltrp_ext
    )
    corestate.init_new_bb()
    bb_gen_success = gen_basicblock(fuzzerstate, corestate, test_params)
    if not bb_gen_success:
        return False
    corestate.basic_blocks[-1].insert(
        -1,
        BranchInstr(
            "beq",
            IntReg.zero,
            corestate.stateloop_cmp_reg,
            0,
            True,
            test_params.is_rv64,
            fixed_opcode=True,
        ),
    )

    if isinstance(corestate.basic_blocks[-1][-1], BranchInstr):
        corestate.basic_blocks[-1][-1] = JALInstr(
            "jal", IntReg.zero, corestate.basic_blocks[-1][-1].imm
        )
    if isinstance(corestate.basic_blocks[-1][-1], JALInstr):
        assert corestate.basic_blocks[-1][-1].imm > -1 << 20
    # accound for extra instruction in PC relative addresses
    if isinstance(corestate.basic_blocks[-1][-1], (JALInstr, BranchInstr)):
        corestate.basic_blocks[-1][-1].imm -= ILEN
    if mtvec_prod is not None:
        corestate.intregpickstate.set_regstate(mtvec_prod, IntRegState.FREE)
    corestate.intregpickstate.set_regstate(
        corestate.stateloop_cmp_reg, IntRegState.FREE
    )
    corestate.stateloop_cmp_reg = None
    corestate.interrupt_block = False
    return True


def steer_to_m_mode(
    fuzzerstate: "FuzzerState", corestate: "CoreState", test_params: "TestParams"
):
    """The final block must be executed in M-mode in order to dump the contents
    of the FPU registers. This function will try to change the priviledge mode
    to M-mode by overwriting the last instruction and replace it with an
    exception
    """

    def revert_last_bb_instr_state(corestate: "CoreState"):
        """reverts the state changes that the last CFI instruction of a basic
        block might have had on the core
        """
        last_instr = corestate.basic_blocks[-1][-1]
        if isinstance(last_instr, JALRInstr):
            corestate.intregpickstate.set_regstate(
                last_instr.rs1, IntRegState.CONSUMED, force=True
            )
        elif isinstance(last_instr, TrapInstr):
            corestate.hartstate.privlvl = last_instr.prev_priv
            if last_instr.is_mtvec:
                assert not corestate.hartstate.is_mtvec_populated
                corestate.hartstate.is_mtvec_populated = True
            else:
                assert not corestate.hartstate.is_stvec_populated
                corestate.hartstate.is_stvec_populated = True
            if isinstance(last_instr, MisalignedMemInstr):
                consumed_reg = last_instr.rs1
                corestate.intregpickstate.set_regstate(
                    consumed_reg, IntRegState.CONSUMED, force=True
                )
        elif isinstance(last_instr, PrivDescentInstr):
            if last_instr.is_mret:
                corestate.hartstate.is_mepc_populated = True
            else:
                corestate.hartstate.is_sepc_populated = True
            corestate.hartstate.privlvl = last_instr.prev_priv

    if corestate.hartstate.privlvl != PrivLvl.Machine:
        revert_last_bb_instr_state(corestate)
        # If we are in M-mode becuase we removed the priv descent, we can use
        # a normal CFI instead of an exception
        if corestate.hartstate.privlvl == PrivLvl.Machine:
            # Account for the instruction we are replacing
            curr_addr = corestate.get_current_addr() - ILEN
            dist_to_fianl_bb = abs(curr_addr - fuzzerstate.get_final_bb_base_addr())
            has_consumed_reg = corestate.intregpickstate.exists_reg_in_state(
                IntRegState.CONSUMED
            )
            cfi_instr = [ISAInstrClass.JAL]
            if has_consumed_reg:
                cfi_instr.append(ISAInstrClass.JALR)
            if dist_to_fianl_bb < (1 << get_range_bits(ISAInstrClass.BRANCH)):
                cfi_instr.append(ISAInstrClass.BRANCH)
            curr_isa_class = test_params.prng.choice(cfi_instr)
            if curr_isa_class == ISAInstrClass.BRANCH:
                corestate.curr_branch_taken = True
            instr_str = test_params.prng.choice(INSTRS_BY_ISA_CLASS[curr_isa_class])
            corestate.next_bb_addr = fuzzerstate.get_final_bb_base_addr()
            instr = create_instr(
                instr_str,
                corestate,
                fuzzerstate,
                test_params.prng,
                curr_addr,
                test_params.is_rv64,
            )
            assert len(instr) == 1
            corestate.basic_blocks[-1][-1] = instr[0]
            return True

        has_consumed_reg = corestate.intregpickstate.exists_reg_in_state(
            IntRegState.CONSUMED
        )
        takable_exceptions = corestate.hartstate.gen_takable_exception_dict(
            test_params, has_consumed_reg
        )
        to_machine = []
        for exception, takable in takable_exceptions.items():
            if takable:
                is_takable_to_m_mode = bool(
                    (~corestate.hartstate.medeleg) >> exception.value & 0b1
                )
                if is_takable_to_m_mode:
                    to_machine.append(exception)

        if not to_machine:
            if not corestate.hartstate.is_stvec_populated:
                return False
            curr_addr = corestate.get_current_addr() - ILEN
            corestate.next_bb_addr = fuzzerstate.get_final_bb_base_addr()
            # TODO this only works with max 20 bit memory
            instr = create_instr(
                "jal",
                corestate,
                fuzzerstate,
                test_params.prng,
                curr_addr,
                test_params.is_rv64,
            )
            assert len(instr) == 1
            corestate.basic_blocks[-1][-1] = instr[0]
        else:
            chosen_exception = test_params.prng.choice(to_machine)
            corestate.basic_blocks[-1][-1] = gen_exception_from_cause(
                corestate, test_params, chosen_exception
            )
    return True


def connect_with_final_block(
    fuzzerstate: "FuzzerState",
    corestate: "CoreState",
    is_rv64: bool,
    prng: Random,
):
    """If the ControlFlow instruction of the last basic block in the current
    sequence is a Branch, it might not be able to reach the final basic block.
    This function checks if the range of the branch is enough, and if not,
    replaces the branch with a JAL. This work under the (safe) assumption that
    the memory is not over 2MiB (1<<20)
    """
    last_instr = corestate.basic_blocks[-1][-1]
    if isinstance(last_instr, BranchInstr):
        last_cfi_addr = corestate.get_current_addr() - ILEN
        offset = fuzzerstate.get_final_bb_base_addr() - last_cfi_addr
        range_bits = get_range_bits(ISAInstrClass.BRANCH)
        last_bb_in_range = -(1 << range_bits) < offset < ((1 << range_bits) - 2)
        if not last_bb_in_range:
            corestate.next_bb_addr = fuzzerstate.get_final_bb_base_addr()
            jal_i = create_instr(
                "jal", corestate, fuzzerstate, prng, last_cfi_addr, is_rv64
            )
            assert len(jal_i) == 1
            corestate.basic_blocks[-1][-1] = jal_i[0]


def gen_memop_addrs(
    fuzzerstate: "FuzzerState",
    corestate: "CoreState",
    mcmstate: "MCMState",
    n_cores: int,
) -> list[int]:
    """return a list of addresses for the memory operations, in their order of
    occurrence
    """
    ret: list[int] = []
    for bb_idx, bb_instrs in enumerate(corestate.basic_blocks):
        for instr_idx, instr in enumerate(bb_instrs):
            if is_placeholder(instr):
                continue
            elif (
                isinstance(
                    instr,
                    (
                        IntLoadInstr,
                        IntStoreInstr,
                        FloatLoadInstr,
                        FloatStoreInstr,
                        AmoInstr,
                        AmoStore,
                    ),
                )
                and instr.producer_id is not None
            ):
                memop_addr = pick_memop_addr(fuzzerstate, instr.instr_str, n_cores)
                ret.append(memop_addr)
                if runparams.FUZZ_MCM:
                    mcmstate.add_tagret_addr(
                        corestate.hartid, memop_addr, bb_idx, instr_idx
                    )
    return ret
