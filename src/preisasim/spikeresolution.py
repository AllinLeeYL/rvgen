# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


from preisasim.spike import (
    verify_pc_trace,
    asymmetric_isa_presim,
    SPIKE_STARTADDR,
)
import runparams
from params import get_design_march_flags_nocompressed
from riscv import ILEN
from instgen import (
    PlaceholderConsumerInstr,
    BranchInstr,
    PlaceholderProducerInstr0,
    PlaceholderProducerInstr1,
    IntStoreInstr,
    FloatStoreInstr,
    AmoInstr,
    AmoStore,
)
from genelf import gen_elf_from_bbs
from typing import TYPE_CHECKING
from random import Random
import os

if TYPE_CHECKING:
    from states import CoreState
    from mcm import MCMState
    from proggen import TestCaseGenerator
    from params import TestParams

###
# Utility functions
###


def gen_regdump_reqs(corestate: "CoreState") -> list[tuple[int, bool, int]]:
    """
    Generate register dump requests based on the core state.

    This function processes the instructions and basic block start addresses
    from the given core state to generate a list of register dump requests.
    Each request contains the address of the instruction and the dependent
    register value at the time of consumption.

    Args:
        corestate (CoreState): Core specific state of the fuzzer

    Returns:
        list: A list of tuples where each tuple contains:
              - The address of the instruction (int)
              - A boolean flag (always False in this context)
              - The dependent register value (int)
    """
    if __debug__:
        assert len(corestate.basic_blocks) == len(corestate.bb_start_addrs)

    ret = []
    for bb_start_addr, bb_instrs in zip(
        corestate.bb_start_addrs, corestate.basic_blocks
    ):
        for instr_id, instr in enumerate(bb_instrs):
            curr_addr = bb_start_addr + ILEN * instr_id  # NO_COMPRESSED
            # All we need is the value of the dependent register at consumption
            # time.
            if isinstance(instr, PlaceholderConsumerInstr):
                ret.append((curr_addr, False, instr.rs2))
            # For branches, we need to know the val of both operands to generate
            # a suitable opcode later.
            if isinstance(instr, BranchInstr) and not instr.fixed_opcode:
                ret.append((curr_addr, False, instr.rs1))
                ret.append((curr_addr, False, instr.rs2))
            # we need the value of stores
            if runparams.FUZZ_MCM and isinstance(
                instr, (IntStoreInstr, AmoInstr, AmoStore)
            ):
                ret.append((curr_addr, False, instr.rs2))
            if runparams.FUZZ_MCM and isinstance(instr, FloatStoreInstr):
                ret.append((curr_addr, True, instr.frs2))
    return ret


# @brief from ..the output of the regdump, distributes the values to the instructions
# @return nothing
def _feed_regdump_to_instrs(
    corestate: "CoreState", mcmstate: "MCMState", regdumps: list, prng: Random
):
    if __debug__:
        assert len(corestate.basic_blocks) == len(corestate.bb_start_addrs)

    producer_id_to_rdepval = dict()
    regdump_idx = 0
    for bb_idx, bb_instrs in enumerate(corestate.basic_blocks):
        # bb_start_addr = corestate.bb_start_addrs[bb_idx] # DEBUG
        for instr_idx, instr in enumerate(bb_instrs):
            # For consumers, we just place the register value for the producers
            if isinstance(instr, PlaceholderConsumerInstr):
                producer_id_to_rdepval[instr.producer_id] = regdumps[regdump_idx]
                regdump_idx += 1

            if isinstance(instr, BranchInstr) and not instr.fixed_opcode:
                branch_rs1_content = regdumps[regdump_idx]  # rs1
                regdump_idx += 1
                branch_rs2_content = regdumps[regdump_idx]  # rs2
                regdump_idx += 1
                # Determine branch opcode based on the register values
                instr.select_suitable_opcode(
                    branch_rs1_content, branch_rs2_content, prng
                )

            if runparams.FUZZ_MCM and isinstance(
                instr, (IntStoreInstr, FloatStoreInstr, AmoStore, AmoInstr)
            ):
                instr_coord = (bb_idx, instr_idx)
                data = regdumps[regdump_idx]
                if isinstance(instr, IntStoreInstr) and instr.is_interrupt:
                    regdump_idx += 1
                    continue
                mcmstate.add_store_data(corestate.hartid, instr_coord, data)
                regdump_idx += 1

    # Feed the consumer-level information into the producers
    for bb_instrlist in corestate.basic_blocks:
        for instr in bb_instrlist:
            if isinstance(instr, PlaceholderProducerInstr0) or isinstance(
                instr, PlaceholderProducerInstr1
            ):
                if instr.producer_id in producer_id_to_rdepval:
                    instr.rtl_offset = (
                        producer_id_to_rdepval[instr.producer_id]
                        ^ instr.spike_resolution_offset
                        ^ SPIKE_STARTADDR
                    )
                else:
                    instr.rtl_offset = instr.spike_resolution_offset


def _transmit_addrs_to_producers_for_spike_resolution(
    corestate: "CoreState", producer_id_to_tgtaddr: dict, prng: Random
):
    for bb_instrlist in corestate.basic_blocks:
        for instr in bb_instrlist:
            if isinstance(instr, PlaceholderProducerInstr0):
                assert instr.producer_id
                if instr.producer_id not in producer_id_to_tgtaddr:
                    producer_id_to_tgtaddr[instr.producer_id] = (
                        prng.randrange(1 << 30) << 2
                    )
                instr.spike_resolution_offset = producer_id_to_tgtaddr[
                    instr.producer_id
                ]
            elif isinstance(instr, PlaceholderProducerInstr1):
                assert instr.producer_id
                instr.spike_resolution_offset = producer_id_to_tgtaddr[
                    instr.producer_id
                ]


# Takes a fuzzerstate after its basic blocks were generated.
# Does the address resolution in place in the fuzzerstate, and returns the list of expected register values.
# @return a list of num_pickable_intregs
def spike_resolution(
    testcase_generator: "TestCaseGenerator",
    test_params: "TestParams",
    skip_pc_smoke_test: bool = False,
):
    design_name = test_params.design_name
    finalintregvals_spikeresol: dict[int, list[int]] = {}
    finalfpuregvals_spikeresol: dict[int, list[int]] = {}

    for hartid, core in testcase_generator.corestates.items():
        producer_id_to_tgtaddr = core.get_producer_id_to_tgtaddr()
        _transmit_addrs_to_producers_for_spike_resolution(
            core, producer_id_to_tgtaddr, test_params.prng
        )

    spike_resolution_elfpath = gen_elf_from_bbs(
        testcase_generator, True, "spikeresol", test_params
    )

    for hartid, core in testcase_generator.corestates.items():
        regdump_reqs = gen_regdump_reqs(core)
        regvals, (core_intregval, core_floatregval, _) = asymmetric_isa_presim(
            test_params,
            spike_resolution_elfpath,
            regdump_reqs,
            testcase_generator.fuzzerstate.get_final_bb_base_addr() + SPIKE_STARTADDR,
            hartid,
        )

        # IMPORTANT: We reset the randomness here to have deterministic branch
        # instructions.
        # (Rare) example where it matters: assume we need to pop the last bb,
        # say with id 20. Then we could have a bug with request size 19 but not with
        # request size 20, or vice versa.
        test_params.prng.seed(test_params.randseed)
        _feed_regdump_to_instrs(
            core, testcase_generator.mcmstate, regvals, test_params.prng
        )
        finalintregvals_spikeresol[hartid] = core_intregval
        finalfpuregvals_spikeresol[hartid] = core_floatregval

    if __debug__ and not skip_pc_smoke_test:
        rtl_spike_elfpath = gen_elf_from_bbs(
            testcase_generator, False, "spikedoublecheck", test_params
        )

        verify_pc_trace(
            test_params.instance_to_str(),
            rtl_spike_elfpath,
            get_design_march_flags_nocompressed(design_name),
            testcase_generator,
            test_params.spike_timeout,
        )

        if runparams.REMOVE_TMPFILES:
            os.remove(rtl_spike_elfpath)
            del rtl_spike_elfpath
        else:
            # print("rtl_spike_elfpath:", rtl_spike_elfpath)
            pass

    if runparams.REMOVE_TMPFILES:
        os.remove(spike_resolution_elfpath)

    return finalintregvals_spikeresol, finalfpuregvals_spikeresol
