# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


import time
from random import Random
from typing import TYPE_CHECKING, Optional
from proggen.util import (
    gen_producer_id_to_tgtaddr,
    blacklist_changing_instructions,
    gen_memop_addrs,
    gen_interrupt_block,
    gen_stateloop,
    steer_to_m_mode,
    connect_with_final_block,
    StressCoreGen,
)
from proggen.blockgen import gen_basicblock
from proggen.instrgen import send_ipi, write_foreign_instr
import runparams
from mcm import MCMState
from states import CoreState, FuzzerState, IntRegState, FloatRegState, SyncState
from riscv import SPIKE_CLINT_BASE, MTIMER_OFFSET, IntReg
from preisasim import SPIKE_STARTADDR
from staticblocks import (
    gen_initial_basic_block,
    gen_finalblock,
    gen_mcm_statereset_handler,
)
from instgen import JALInstr
from params import LIMIT_MEM_SATURATION_RATIO, MAX_NUM_PICKABLE_INTREGS

if TYPE_CHECKING:
    from params import TestParams


class TestCaseGenerator:
    """Wrapper class to store the fuzzer state and the list of corestates"""

    # Class constants
    MAX_FINAL_BLOCK_RETRIES = 5
    ALIGNMENT_BITS_MAX = 3  # quad RISC-V extensions not supported
    OFFSET_RANGE_BITS = 11  # 2^11 byte range for offset addresses

    def __init__(self, test_params: "TestParams"):
        """
        Creates a fuzzerstate instance for the current programs, and one
        corestate instance per core
        """
        self.mcmstate: "MCMState" = MCMState(
            test_params.hartids, test_params.is_rv64, test_params.design_has_fpud
        )
        self.fuzzerstate: "FuzzerState" = FuzzerState(test_params)
        self.corestates: dict[int, "CoreState"] = {}
        for hartid in test_params.hartids:
            self.corestates[hartid] = CoreState(test_params, hartid)
        self.num_addr_regs = self.get_num_addr_registers(
            self.fuzzerstate.num_store_locs
        )
        for hartid in test_params.hartids:
            self.corestates[hartid].pick_addr_regs(self.num_addr_regs, test_params.prng)

        # Generate the static blocks (non-fuzzing & testcase independant)
        init_ok = gen_initial_basic_block(
            self.fuzzerstate, self.corestates, test_params
        )
        if __debug__:
            assert init_ok, "failed to generate init block, memsize likely too small"
        for _ in range(self.MAX_FINAL_BLOCK_RETRIES):
            final_ok = gen_finalblock(self.fuzzerstate, test_params)
            if final_ok:
                break
        else:
            raise RuntimeError("Failed to generate initial or final basic block")

        # Values generated
        self.rtl_elfpath: str = ""
        self.expected_regvals: tuple[dict[int, list[int]], dict[int, list[int]]] = (
            {},
            {},
        )

        # Statistics
        self.seconds_spent_in_gen_bbs: float = 0
        self.seconds_spent_in_spike_resol: float = 0
        self.seconds_spent_in_gen_elf: float = 0
        self.seconds_spent_verifying_output: float = 0
        self.seconds_spent_in_rtl_sim: float = 0

    def gen_program(self, test_params: "TestParams"):
        """
        Generates all basic blocks for a test case. High-level orchestration method
        that coordinates the entire test case generation process.
        """
        testcase_generation_success = False
        n_attempt = 0
        while not testcase_generation_success:
            testcase_generation_success = self._attempt_testcase_generation(test_params)
            if not testcase_generation_success:
                n_attempt += 1
            if n_attempt > 20:
                return False
            if not testcase_generation_success:
                self.__init__(test_params)  # reset states
        self._finalize_testcase_generation(test_params)
        return True

    def _attempt_testcase_generation(self, test_params: "TestParams") -> bool:
        """
        Attempts to generate a single testcase. Returns True if successful, False if
        the testcase should be discarded and regeneration attempted.
        """
        self._initialize_generation_state(test_params)
        if not self._generate_fuzzing_blocks(test_params):
            return False
        if not self._validate_and_filter_testcase(test_params):
            return False
        if not self._prepare_mode_transition(test_params):
            return False
        self._connect_final_blocks(test_params)
        return True

    def _initialize_generation_state(self, test_params: "TestParams"):
        """Prepares the state for a new testcase generation attempt."""
        # Save the initial register states
        for core in self.corestates.values():
            core.save_reg_state()
        if __debug__:
            assert self.get_total_fuzzing_instrs() == 0
            assert not self.has_reached_max_instr_num(test_params.nmax_instrs)
        # Allocate some addresses for stores
        if runparams.USE_ADDR_REGS:
            self.init_store_locations(test_params.prng)
        else:
            self.fuzzerstate.init_store_locations()

    def _generate_fuzzing_blocks(self, test_params: "TestParams") -> bool:
        """
        Generates fuzzing basic blocks for all cores. Returns True if generation
        completed successfully, False if it should be retried.
        """
        generation_over = [False] * test_params.num_harts

        while not all(generation_over):
            for core in self.corestates.values():
                if not self._should_generate_for_core(core, generation_over):
                    continue
                success = self._generate_core_basic_block(core, test_params)
                self._handle_block_generation_result(
                    core, success, generation_over, test_params
                )

        return True

    def _should_generate_for_core(
        self, core: "CoreState", generation_over: list[bool]
    ) -> bool:
        """Determines if we should generate a basic block for the given core."""
        # Skip if generation is over for this core
        if generation_over[core.hartid]:
            return False

        # Skip if core is waiting for MCM sync and no other cores are done
        if runparams.FUZZ_MCM and self.mcm_complexity_stall(
            core.hartid, generation_over
        ):
            return False

        return True

    def _generate_core_basic_block(
        self, core: "CoreState", test_params: "TestParams"
    ) -> bool:
        """Generates a basic block for a single core and handles additional blocks."""
        core.init_new_bb()
        bb_gen_success = gen_basicblock(self.fuzzerstate, core, test_params)
        self.update_global_sync_state()

        # Generate additional blocks if needed
        if core.interrupt_block:
            bb_gen_success = gen_interrupt_block(core, self.fuzzerstate, test_params)

        if core.state_preserving_loop:
            bb_gen_success = gen_stateloop(core, self.fuzzerstate, test_params)
        return bb_gen_success

    def _handle_block_generation_result(
        self,
        core: "CoreState",
        success: bool,
        generation_over: list[bool],
        test_params: "TestParams",
    ):
        """Handles the result of basic block generation for a core."""
        if success:
            self.update_num_fuzzing_instrs()
            core.save_reg_state()
            if self.is_generation_over(test_params.nmax_bbs, test_params.nmax_instrs):
                generation_over[core.hartid] = True
        else:
            generation_over[core.hartid] = True

    def _validate_and_filter_testcase(self, test_params: "TestParams") -> bool:
        """
        Validates the generated testcase and applies filters.
        Returns False if the testcase should be discarded.
        """
        # emergency basic block to unlock locked cores
        if self.fuzzerstate.waiting_core_ids:
            for core in self.corestates.values():
                if core.hartid not in self.fuzzerstate.waiting_core_ids:
                    if not core.next_bb_addr:
                        print("must abort")
                        return False
                    core.init_new_bb()
                    self.fuzzerstate.n_ipi_sent += 1
                    new_instrobjs = send_ipi(
                        self.fuzzerstate, core, test_params.is_rv64, test_params.prng
                    )
                    new_instrobjs.append(JALInstr("jal", IntReg.zero, 0))
                    core.add_instr(new_instrobjs)
                    break

        if any(
            state == SyncState.WRITE_INSTR
            for state in self.fuzzerstate.sync_states.values()
        ):
            for core in self.corestates.values():
                if self.fuzzerstate.sync_states[core.hartid] == SyncState.FREE:
                    if not core.next_bb_addr:
                        print("must abort")
                        return False
                    core.init_new_bb()
                    new_instrobjs = write_foreign_instr(
                        test_params, self.fuzzerstate, core
                    )
                    new_instrobjs.append(JALInstr("jal", IntReg.zero, 0))
                    core.add_instr(new_instrobjs)
                    break

        # Skip programs without interrupts if enabled
        no_interrupts = (
            self.fuzzerstate.n_ipi_sent + self.fuzzerstate.n_local_interrupts_sent
        ) == 0
        if runparams.SKIP_NO_INTERRUPT and no_interrupts:
            return False
        return True

    def _prepare_mode_transition(self, test_params: "TestParams") -> bool:
        """
        Prepares the transition to M-mode. Returns False if transition cannot be made.
        """
        for core in self.corestates.values():
            can_switch = steer_to_m_mode(self.fuzzerstate, core, test_params)
            if not can_switch:
                return False
        return True

    def _connect_final_blocks(self, test_params: "TestParams"):
        """Connects each core with the final block."""
        for core in self.corestates.values():
            connect_with_final_block(
                self.fuzzerstate, core, test_params.is_rv64, test_params.prng
            )

    def _finalize_testcase_generation(self, test_params: "TestParams"):
        """Finalizes the testcase generation by setting up memory operations and MCM."""
        blacklist_changing_instructions(
            self.fuzzerstate, list(self.corestates.values())
        )

        if runparams.FUZZ_MCM:
            self.mcmstate.register_memory_operations(self.corestates)
        if runparams.FUZZ_STRESS_CORE:
            stress_core = StressCoreGen(test_params, self.fuzzerstate)
            self.fuzzerstate.stress_bb = stress_core.gen_stress_core(
                self.fuzzerstate, test_params
            )

        self._setup_memory_operations()

        if runparams.FUZZ_MCM:
            self.gen_mcmhandler(test_params)

    def _setup_memory_operations(self):
        """Sets up memory operation addresses for each core."""
        for core in self.corestates.values():
            memop_addrs: list[int] = []
            if not runparams.USE_ADDR_REGS:
                memop_addrs += gen_memop_addrs(
                    self.fuzzerstate, core, self.mcmstate, len(self.corestates)
                )
            core.producer_id_to_tgtaddr = gen_producer_id_to_tgtaddr(
                core, memop_addrs, self.fuzzerstate.get_final_bb_base_addr()
            )

    def validate_output(self, test_params: "TestParams"):
        """Validates the execution against the MCM, wrapper to gather
        execution time data
        """
        self.mcmstate.validate_outcome(
            test_params, self.corestates, self.fuzzerstate.store_locations
        )

    def get_total_fuzzing_instrs(self):
        """counts the number of generated intruction"""
        n_fuzzing_instrs = 0
        for core in self.corestates.values():
            n_fuzzing_instrs += sum([len(bb) for bb in core.basic_blocks])
        return n_fuzzing_instrs

    def update_num_fuzzing_instrs(self):
        """Updates the number of total fuzzing instructions generated in all cores"""
        total_fuzz_instr = self.get_total_fuzzing_instrs()
        for core in self.corestates.values():
            core.total_fuzzing_instrs = total_fuzz_instr

    def has_reached_max_instr_num(self, nmax_instrs: Optional[int]):
        """
        returns true if the current program has reached the maximal number
        of instructions
        """
        if nmax_instrs is None:
            return False
        else:
            return self.get_total_fuzzing_instrs() >= nmax_instrs

    def is_generation_over(self, nmax_bbs: Optional[int], nmax_instrs: Optional[int]):
        """
        Returns true is we reached the maximal programs lenght we are allowed
        to produce, either because of normal constraints, or input parameters
        """
        total_n_bb = 0
        for core in self.corestates.values():
            total_n_bb += len(core.basic_blocks)
        if nmax_bbs is None:
            reached_max_bb = False
        else:
            reached_max_bb = total_n_bb >= nmax_bbs
        mem_saturated = (
            self.fuzzerstate.memstate.get_allocated_ratio()
            >= LIMIT_MEM_SATURATION_RATIO
        )
        is_over = (
            self.has_reached_max_instr_num(nmax_instrs)
            or reached_max_bb
            or mem_saturated
        )
        return is_over

    def are_regs_polluted(self):
        """returns True if some regs are polluted"""
        for core in self.corestates.values():
            intreg_polluted = core.intregpickstate.exists_reg_in_state(
                IntRegState.POLLUTED
            )
            floatreg_polluted = core.floatregpickstate.exists_reg_in_state(
                FloatRegState.POLLUTED
            )
            if intreg_polluted or floatreg_polluted:
                return True
        return False

    def prepare_testcase_for_rtl(self, test_params: "TestParams"):
        """If the ELF for rtl simulation is generated, generate the final block with
        the design specific stop mechanism and replace the right clint addresses
        """
        if runparams.DEBUG_RTL:
            return
        if test_params.clint_addr != SPIKE_CLINT_BASE:
            fuzzerstate = self.fuzzerstate
            gen_finalblock(self.fuzzerstate, test_params, is_rtl=True)
            if fuzzerstate.mtimer_reset_idx:
                lui_imm = (test_params.clint_addr + MTIMER_OFFSET) >> 12
                addr_instr = fuzzerstate.init_bb_instrs[fuzzerstate.mtimer_reset_idx]
                addr_instr.imm = lui_imm  # type: ignore[attr-defined]
            lui_imm = test_params.clint_addr >> 12
            for core in self.corestates.values():
                for bb_id, instr_id in core.clint_instr:
                    addr_instr = core.basic_blocks[bb_id][instr_id]
                    addr_instr.imm = lui_imm  # type: ignore[attr-defined]

    def update_global_sync_state(self):
        """Free the cores once all cores reached the MCM synchronization point"""
        if all(
            sync_state == SyncState.MCM
            for sync_state in self.fuzzerstate.sync_states.values()
        ):
            for hartid in self.fuzzerstate.sync_states.keys():
                self.fuzzerstate.sync_states[hartid] = SyncState.FREE

    def mcm_complexity_stall(self, hartid: int, generation_over: list[bool]):
        """Stall cores that are waiting for other cores to reach the
        synchronization point.
        If a core is done, it will unlock all other cores. In this case, we
        stop stalling stalled cores. The core that is done will stay free, and
        the core unstalled will stay in the MCM lock state to avoid trying to
        sync again
        """
        if self.fuzzerstate.sync_states[hartid] == SyncState.FREE:
            return False  # no stall needed
        elif any(generation_over):
            return False  # no stall needed
        else:
            return True  # stall

    def gen_mcmhandler(self, test_params: "TestParams"):
        """Generates the mcm reset handler, must be called once the store
        locations are known"""
        gen_mcm_statereset_handler(self.fuzzerstate, test_params)

    def get_num_addr_registers(self, num_store_locs: int) -> int:
        """returns the number of unsed register.
        This is the minimum number of unsued register across cores
        """
        unused_reg_per_core: list[int] = []
        all_regs = set(list(IntReg)[1 : MAX_NUM_PICKABLE_INTREGS + 1])
        for core in self.corestates.values():
            used_regs = set(core.intregpickstate.pickable_intregs)
            unused_regs = all_regs - used_regs
            unused_reg_per_core.append(len(unused_regs))
            assert len(unused_regs) != 0
        num_unused_regs = min(unused_reg_per_core)
        # Either there are enough regs for 1 per addr, or we use offsets
        return min(num_store_locs, num_unused_regs)

    def init_store_locations(self, prng: Random):
        """Initialize the addresses and assign the addr/reg dictionary to corestates."""
        store_locations = self._allocate_base_store_locations()
        offsetted_addr = self._allocate_offsetted_locations(prng, store_locations)
        self._assign_addresses_to_registers(store_locations, offsetted_addr)
        self._initialize_register_data(store_locations)

    def _allocate_base_store_locations(self) -> list[int]:
        """Allocate one fresh address per address register."""
        memstate = self.fuzzerstate.memstate
        blacklist_memstate = self.fuzzerstate.memview_blacklist
        store_locations = []

        for _ in range(self.num_addr_regs):
            next_store_loc = memstate.gen_random_free_addr(
                self.ALIGNMENT_BITS_MAX,
                1 << self.ALIGNMENT_BITS_MAX,
                0,
                memstate.memsize,
            )
            if next_store_loc is None:
                raise ValueError(
                    f"Could not find a next store location. You may want to increase the memory size (for the moment: {memstate.memsize:,} B)"
                )
            memstate.alloc_mem_range(next_store_loc, (1 << self.ALIGNMENT_BITS_MAX))
            # FIXME atomics, for now we blacklist all store locations
            blacklist_memstate.alloc_mem_range(
                next_store_loc, (1 << self.ALIGNMENT_BITS_MAX)
            )
            store_locations.append(next_store_loc)

        return store_locations

    def _allocate_offsetted_locations(
        self, prng: Random, store_locations: list[int]
    ) -> dict[int, list[int]]:
        """Allocate additional addresses using offsets from base locations."""
        memstate = self.fuzzerstate.memstate
        blacklist_memstate = self.fuzzerstate.memview_blacklist
        num_remaining_addr = self.fuzzerstate.num_store_locs - self.num_addr_regs
        offsetted_addr: dict[int, list[int]] = {}

        for _ in range(num_remaining_addr):
            # Select a base address
            addr = prng.choice(store_locations)
            # Select an address in the range
            low = addr - (1 << self.OFFSET_RANGE_BITS)
            high = addr + ((1 << self.OFFSET_RANGE_BITS) - 1)
            new_addr = memstate.gen_random_free_addr(
                self.ALIGNMENT_BITS_MAX, 1 << self.ALIGNMENT_BITS_MAX, low, high
            )
            if new_addr is None:
                raise ValueError(
                    f"Could not find a next store location. You may want to increase the memory size (for the moment: {memstate.memsize:,} B)"
                )
            memstate.alloc_mem_range(new_addr, (1 << self.ALIGNMENT_BITS_MAX))
            # FIXME atomics, for now we blacklist all store locations
            blacklist_memstate.alloc_mem_range(new_addr, (1 << self.ALIGNMENT_BITS_MAX))
            offset = new_addr - addr
            if addr not in offsetted_addr:
                offsetted_addr[addr] = []
            offsetted_addr[addr].append(offset)

        return offsetted_addr

    def _assign_addresses_to_registers(
        self, store_locations: list[int], offsetted_addr: dict[int, list[int]]
    ):
        """Assign addresses to registers across all cores."""
        # Identify shared and unique registers - make deterministic by sorting
        shared_regs: set[IntReg] = set.intersection(
            *(set(core.addr_regs) for core in self.corestates.values())
        )
        # Convert to sorted list for deterministic iteration
        shared_regs_list = sorted(shared_regs, key=lambda reg: reg.value)

        unique_regs_per_core = {}
        for core in self.corestates.values():
            unique_regs = list(set(core.addr_regs) - shared_regs)
            unique_regs_per_core[core.hartid] = sorted(
                unique_regs, key=lambda reg: reg.value
            )

        for core in self.corestates.values():
            self._assign_shared_registers(
                core, shared_regs_list, store_locations, offsetted_addr
            )
            self._assign_unique_registers(
                core,
                unique_regs_per_core[core.hartid],
                store_locations,
                offsetted_addr,
                len(shared_regs_list),
            )

        self._validate_address_assignments()

    def _assign_shared_registers(
        self,
        core: "CoreState",
        shared_regs: list[IntReg],
        store_locations: list[int],
        offsetted_addr: dict[int, list[int]],
    ):
        """Assign shared registers to the same addresses across cores.
        e.g. the same reg id is used in multiple cores
        """
        for addr_idx, reg in enumerate(shared_regs):
            addr = store_locations[addr_idx]
            core.addr_reg_pairs[addr] = (reg, 0)
            # Add the addresses that need offset from that reg
            if addr in offsetted_addr:
                for offset in offsetted_addr[addr]:
                    core.addr_reg_pairs[addr + offset] = (reg, offset)

    def _assign_unique_registers(
        self,
        core: "CoreState",
        unique_regs: list[IntReg],
        store_locations: list[int],
        offsetted_addr: dict[int, list[int]],
        shared_regs_offset: int,
    ):
        """Assign unique registers to addresses for a specific core."""
        for idx, reg in enumerate(unique_regs):
            addr_idx = idx + shared_regs_offset
            addr = store_locations[addr_idx]
            core.addr_reg_pairs[addr] = (reg, 0)
            if addr in offsetted_addr:
                for offset in offsetted_addr[addr]:
                    core.addr_reg_pairs[addr + offset] = (reg, offset)

    def _validate_address_assignments(self):
        """Validate that all cores have the same number of address assignments."""
        if __debug__:
            # Process cores in deterministic order by hartid
            pair_lengths = [
                len(core.addr_reg_pairs) for core in self.corestates.values()
            ]
            assert all(length == pair_lengths[0] for length in pair_lengths)

    def _initialize_register_data(self, store_locations: list[int]):
        """Set the initial block values for address registers."""
        all_reg_to_addr: dict[IntReg, int] = {}
        all_pickable_intregs = list(
            {
                reg
                for core in self.corestates.values()
                for reg in core.intregpickstate.pickable_intregs
                if reg != IntReg.zero
            }
        )
        all_pickable_floatregs = list(
            {
                reg
                for core in self.corestates.values()
                for reg in core.floatregpickstate.pickable_floatregs
            }
        )

        # Build register to address mapping
        for core in self.corestates.values():
            for addr, (reg, offset) in core.addr_reg_pairs.items():
                if offset != 0:
                    continue
                all_reg_to_addr[reg] = addr

        # Set initial register data
        for idx, reg in enumerate(self.fuzzerstate.all_addr_regs):
            data_idx = idx + len(all_pickable_floatregs) + len(all_pickable_intregs)
            self.fuzzerstate.initial_reg_data_content[data_idx] = (
                all_reg_to_addr[reg] + SPIKE_STARTADDR
            )

        # Validate and set store locations
        if __debug__:
            for core in self.corestates.values():
                assert list(core.addr_reg_pairs.keys()) == list(
                    self.corestates[0].addr_reg_pairs.keys()
                )
        self.fuzzerstate.store_locations = list(
            self.corestates[0].addr_reg_pairs.keys()
        )
