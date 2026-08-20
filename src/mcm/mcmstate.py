# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


import time
from typing import TYPE_CHECKING
from mcm.ppo_rules import PpoRules
from mcm.types import Addr, HartId, InstrCoord
from riscv.rvwmo import MEMOP
from mcm.memops import Store, Load, Amo
from mcm.solve_steps import SolveStep
from mcm.step import Step
from mcm.dartagnan import DartagnanSolve
from preisasim import SPIKE_STARTADDR
from instgen import IntStoreInstr, IntLoadInstr
from riscv import ILEN, SPIKE_CLINT_BASE
import runparams
from instgen import (
    FenceInstr,
    WrapperInstr,
    MisalignedMemInstr,
    TrapInstrWrapper,
)

if TYPE_CHECKING:
    from states.corestate import CoreState
    from params import TestParams


# TODO the handler could avoid zeroing out the addresses, as the address
# will take the value of the previous store of A hart of the previous section


class MCMState:
    """
    This class keeps track of the possible return values for a given address
    during the generation of a test case.

    We do not register memory operations from the initial and final block, as well
    as operations from memory operations to isolated special purpose address like
    the synchronization address and memory mapped registers
    """

    def __init__(self, hartids: list[HartId], is_rv64: bool, has_fpud: bool):
        """all orderings here are split into the different reset steps"""
        self.is_rv64: bool = is_rv64
        self.has_fpud: bool = has_fpud
        self.steps: list[Step] = []
        self.instr_coord_to_step: dict[HartId, dict[InstrCoord, int]] = {}
        self.outcome: dict[Addr, int] = {}
        for hartid in hartids:
            self.instr_coord_to_step[hartid] = {}

    def get_all_load_addrs(self):
        """return the execution address of all loads of the program"""
        ret = []
        for step in self.steps:
            for all_memops in step.step_memops.values():
                for memop in all_memops.values():
                    if isinstance(memop, Load):
                        ret.append(memop.exec_addr + SPIKE_STARTADDR)
                    if isinstance(memop, Amo):
                        ret.append(memop.exec_addr + SPIKE_STARTADDR)
        return ret

    def register_memory_operations(self, corestates: dict[HartId, "CoreState"]):
        """adds instructions to the mcm program view, must be done once the
        testcase is fully generated
        """
        for hartid, corestate in corestates.items():
            all_memops, all_fences = self._get_all_mcminstr(corestate)
            if __debug__:
                assert len(all_memops) == len(all_fences), "step count does not match"
            # add potentially missing steps if other core has less memops
            if len(all_memops) > len(self.steps):
                for _ in range(len(all_memops) - len(self.steps)):
                    step = Step(list(corestates.keys()))
                    self.steps.append(step)
            for step_idx, step_memops in enumerate(all_memops):
                step = self.steps[step_idx]
                for (
                    instr,
                    exec_addr,
                    instr_coord,
                    dest_addr,
                ) in step_memops:
                    step.add_memop(
                        instr,
                        hartid,
                        exec_addr,
                        instr_coord,
                        dest_addr,
                        step_idx,
                    )
            for step_idx, step_fences in enumerate(all_fences):
                step = self.steps[step_idx]
                for instr, exec_addr, instr_coord, prev_memop_idx in step_fences:
                    step.add_fence_instruction(
                        instr, hartid, exec_addr, instr_coord, step_idx, prev_memop_idx
                    )

    def add_tagret_addr(
        self, hartid: HartId, dest_addr: Addr, bb_idx: int, instr_idx: int
    ):
        """adds the destination address of the memory operation"""
        step_idx = self.instr_coord_to_step[hartid][(bb_idx, instr_idx)]
        memop = self.steps[step_idx].step_memops[hartid][(bb_idx, instr_idx)]
        memop.add_dest_addr(dest_addr)

    def add_store_data(self, hartid: HartId, instr_coord: InstrCoord, data: int):
        """adds the data stored by a store instruction"""
        step_idx = self.instr_coord_to_step[hartid][instr_coord]
        memop = self.steps[step_idx].step_memops[hartid][instr_coord]
        if isinstance(memop, Store):
            memop.add_data(data)
        elif isinstance(memop, Amo):
            memop.add_data_rs2(data)

    def validate_outcome(
        self,
        test_params: "TestParams",
        corestates: dict[HartId, "CoreState"],
        memop_addrs: list[int],
    ):
        if self.outcome == {}:
            # no load and stores in that section
            return
        if runparams.VERIFY_MEMORY_TRACE:
            # get all ppo constraints
            self._gather_ppo_constraints(corestates, only_syntactic_deps=True)
            # validate all steps
            self._verify_trace(corestates, test_params.instance_to_str())
        else:
            self._gather_ppo_constraints(corestates)
            # First compute the set of valid outcomes
            self._compute_memory_orderings(memop_addrs)
            # Then, for each load store pairs, compute the actual load value
            self._resolve_loads()
            # Finally, verify that the outcome is valid
            self._verify_output(test_params)

    def _verify_trace(self, corestates: dict[HartId, "CoreState"], test_id: str):
        addr_regs = {}
        for hartid, core in corestates.items():
            addr_regs[hartid] = core.addr_reg_pairs
        for idx, step in enumerate(self.steps):
            solver = DartagnanSolve(
                step.step_memops,
                step.step_fences,
                addr_regs,
                self.outcome,
                idx,
                test_id,
            )
            solver.gen_litmus()
            assert runparams.VERIFY_MEMORY_TRACE
            solver.run_litmus(repr(step), backend=runparams.VERIFY_MEMORY_TRACE)

    def _gather_ppo_constraints(
        self, corestates: dict[HartId, "CoreState"], only_syntactic_deps: bool = False
    ):
        for step in self.steps:
            ppo_constraints = PpoRules(step.step_memops, step.step_fences)
            ppo_constraints.gather_all_ppo_constraints(corestates, only_syntactic_deps)

    def _compute_memory_orderings(self, memop_addrs: list[int]):
        """Computes all possible global orderings for the current programs.
        First, all PPO rules are collected, on each cores.
        Then, given the PPO constraints, the possible ordering of each hart is
        computed.
        Finally, the global program orders are computed by interleaving the
        ordering of each harts.
        """
        # Gather the ordering
        for step_idx, step in enumerate(self.steps):
            # print(step)
            solver = SolveStep(step_idx, memop_addrs, step.step_memops)
            # start = time.time()
            step.all_rf_maps = solver.compute_execution_graph()
            # elapsed = time.time() - start
            # print(f"Step {step_idx}: compute_execution_graph() took {elapsed:.4f} seconds")

    def _resolve_loads(self):
        """Compute all set of values that can be returned by a load for each
        step. Handles NaN boxing for floating point values, and signess for
        interger load/stores. Handles allignemnt as well.

        Must be called after the pre-ISA simulation

        !!! WARNING this function works under the assumption that the previous
        store always access a memory size that is equal or larger than the
        current load operation. Else we must compute the 2 previous load to the
        address, which is too complex for us now -> FUTURE IMPL
        """
        for step in self.steps:
            step.compute_all_return_sequences(self.is_rv64, self.has_fpud)

    def _verify_output(self, test_params: "TestParams"):
        if self.steps == []:
            return True
        for step_idx, step in enumerate(self.steps):
            # Check if any hart has load-store pairs
            has_sequences = any(len(sequences) > 0 for sequences in step.all_rf_maps)
            if not has_sequences:
                continue
            found_match = False
            # gather only the relevant addresses for the current step
            filtered_outcome = {
                k: self.outcome[k] for k in step.pc_to_load_return[0].keys()
            }
            for step_outcome in step.pc_to_load_return:
                if len(step_outcome) == len(filtered_outcome) and all(
                    addr in filtered_outcome and filtered_outcome[addr] == val
                    for addr, val in step_outcome.items()
                ):
                    found_match = True
                    break
            if not found_match:
                # Collect debug information
                debug_info = []
                debug_info.append(f"mismatch in step {step_idx}/{len(self.steps)}:")
                debug_info.append("** Expected **")
                for outcome in self.steps[step_idx].pc_to_load_return:
                    debug_info.append("------")
                    for addr, data in outcome.items():
                        debug_info.append(f"@ pc: {hex(addr)} => {hex(data)}")
                debug_info.append("** Actual output **")
                for addr, data in filtered_outcome.items():
                    debug_info.append(f"@ pc: {hex(addr)} => {hex(data)}")
                debug_info.append("** Instrs **")
                debug_info.append(repr(self.steps[step_idx]))

                # Print to console
                for line in debug_info:
                    print(line)

                # Create detailed error message with debug info
                error_details = "\n".join(debug_info)
                raise ValueError(
                    f"MCM mismatch for testcase {test_params.instance_to_str()}\n"
                    f"Debug details:\n{error_details}"
                )
        assert step_idx + 1 == len(self.steps), (step_idx, len(self.steps))
        return True

    def _get_all_mcminstr(self, corestate: "CoreState"):
        """Checks if the instruction stream has a instruction relevant to memory
        ordering and split the operations into the steps.
        Instead of making a step range list, store the fence and memops in lists of lists
        reflecting the step they are in.
        """

        memop_steps: list[list[tuple]] = []
        fence_steps: list[list[tuple]] = []
        curr_memop_step: list[tuple] = []
        curr_fence_step: list[tuple] = []
        hartid: HartId = corestate.hartid

        def add_instruction(instr, instr_coord, curr_addr):
            # Wrapped instruction, and interrutps are never overlapping and
            # therefore doe not play a role in solving
            if isinstance(instr, (IntLoadInstr, IntStoreInstr)) and instr.is_interrupt:
                return
            if isinstance(instr, WrapperInstr):
                return
            # Handle memory operations
            elif isinstance(instr, MEMOP):
                dest_addr = None
                if runparams.USE_ADDR_REGS:
                    dest_addr = instr.dest_addr
                curr_memop_step.append((instr, curr_addr, instr_coord, dest_addr))

            # Handle fence instructions
            elif isinstance(instr, FenceInstr) and instr.instr_str != "fence.i":
                prev_memop_idx = len(curr_memop_step) - 1
                curr_fence_step.append((instr, curr_addr, instr_coord, prev_memop_idx))

        step_idx = 0
        for bb_idx, bb_instrs in enumerate(corestate.basic_blocks):
            bb_start_addr = corestate.bb_start_addrs[bb_idx]
            for instr_idx, instr in enumerate(bb_instrs):
                instr_coord = (bb_idx, instr_idx)
                curr_addr = bb_start_addr + ILEN * instr_idx

                # Handle synchronization steps
                if instr_coord in corestate.mcm_reset_locs:
                    if __debug__:
                        assert curr_memop_step
                    # Assign step index to all instr_coords in this step
                    for _, _, coord, _ in curr_memop_step:
                        self.instr_coord_to_step[hartid][coord] = step_idx
                    for _, _, coord, _ in curr_fence_step:
                        self.instr_coord_to_step[hartid][coord] = step_idx
                    memop_steps.append(curr_memop_step)
                    fence_steps.append(curr_fence_step)
                    curr_memop_step = []
                    curr_fence_step = []
                    step_idx += 1
                add_instruction(instr, instr_coord, curr_addr)

        # Handle last step, ignore if there are only fences
        if curr_memop_step:
            for _, _, coord, _ in curr_memop_step:
                self.instr_coord_to_step[hartid][coord] = step_idx
            for _, _, coord, _ in curr_fence_step:
                self.instr_coord_to_step[hartid][coord] = step_idx
            memop_steps.append(curr_memop_step)
            fence_steps.append(curr_fence_step)

        return memop_steps, fence_steps

    def __repr__(self) -> str:
        """Debug state representation"""
        ret = ""
        for step in self.steps:
            ret += repr(step)
            # Only add "synchronization step" if not the last step
            if step is not self.steps[-1]:
                ret += "synchronization step\n"
        return ret
