# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


from z3 import Solver, Int, Implies, Xor, And, Distinct
from typing import TYPE_CHECKING, Optional, Tuple, TypeAlias, DefaultDict
from collections import defaultdict, deque
from copy import deepcopy

from mcm.types import Addr, HartId, InstrCoord, MemOrdering
from riscv.rvwmo import MEMOP, STORE_INSTR, LOAD_INSTR, AMO_INSTR
from mcm.memops import MemFence, MemInstr, Store, Load, Amo, make_syntethic_init_store
from instgen import FenceInstr
from preisasim import SPIKE_STARTADDR

if TYPE_CHECKING:
    from instgen import RVInstr


# Private type aliases local to this module
PoIdx = int
Node = tuple[HartId, PoIdx]


class Step:
    """Store a single MCM step memory instruction. A solving steps contains all
    memory instructions before a reset of the state of the MCM.
    Computes the return value pairs once the orderings have been computed
    """

    def __init__(self, hartids: list[HartId]):
        # gathered from testcase
        self.step_memops: dict[HartId, dict[InstrCoord, MemInstr]] = {}
        self.step_fences: dict[HartId, dict[InstrCoord, MemFence]] = {}

        # computed using solver output
        self.all_rf_maps: list[dict[Load | Amo, Optional[Store | Amo]]] = []
        self.pc_to_load_return: list[dict[Addr, int]] = []
        for hartid in hartids:
            self.step_memops[hartid] = {}
            self.step_fences[hartid] = {}

    def add_memop(
        self,
        mem_instr: "RVInstr",
        hartid: HartId,
        exec_addr: Addr,
        instr_coord: InstrCoord,
        dest_addr: Optional[Addr],
        step: int,
    ):
        """Saves a new memory instruction in the memory instruction tracking
        infrastructure
        """
        assert isinstance(mem_instr, MEMOP)
        idx = len(self.step_memops[hartid])
        if isinstance(mem_instr, LOAD_INSTR):
            memop = Load(
                mem_instr.instr_str,
                exec_addr,
                idx,
                instr_coord,
                dest_addr,
                hartid,
                step,
            )
        elif isinstance(mem_instr, STORE_INSTR):
            memop = Store(
                mem_instr.instr_str,
                exec_addr,
                idx,
                instr_coord,
                dest_addr,
                hartid,
                step,
            )
        elif isinstance(mem_instr, AMO_INSTR):
            memop = Amo(
                mem_instr.instr_str,
                exec_addr,
                idx,
                instr_coord,
                dest_addr,
                hartid,
                step,
                mem_instr.ordering,
            )
        else:
            raise ValueError

        self.step_memops[hartid][instr_coord] = memop

    def add_fence_instruction(
        self,
        fence_instr: FenceInstr,
        hartid: HartId,
        exec_addr: Addr,
        instr_coord: InstrCoord,
        step: int,
        prev_memop_idx: int,
    ):
        """adds a fence to the list of fences"""
        memfence = MemFence(
            exec_addr, fence_instr.fence_ordering, prev_memop_idx, instr_coord, step
        )
        self.step_fences[hartid][instr_coord] = memfence

    def compute_all_return_sequences(self, is_rv64: bool, has_fpud: bool):
        pc_to_load_return: list[dict[int, int]] = []
        for ordering in self.all_rf_maps:
            step_pc_to_load_return = self._compute_return_sequence(
                ordering, is_rv64, has_fpud
            )
            if step_pc_to_load_return not in pc_to_load_return:
                pc_to_load_return.append(step_pc_to_load_return)
        self.pc_to_load_return = pc_to_load_return

    def _compute_return_sequence(
        self,
        ordering: dict[Load | Amo, Store | Amo | None],
        is_rv64: bool,
        has_fpud: bool,
    ):
        """Given an ordering, compute the sequence of load return values.
        Returns a dictionary mapping the PC address of the load operations to
        their return values
        """
        step_pc_to_load_return = {}
        pending_dependencies: dict[Store | Amo, list[Load | Amo]] = {}
        # Pass 1: Compute AMO store data and track dependencies
        for load, prev_store in ordering.items():
            if not isinstance(load, Amo):
                continue
            if prev_store is not None:
                if prev_store.get_data() is None:
                    if prev_store not in pending_dependencies:
                        pending_dependencies[prev_store] = []
                    pending_dependencies[prev_store].append(load)
                else:
                    data = self._get_data(load, prev_store, is_rv64, has_fpud)
                    load.compute_store_data(data, is_rv64)
                    # Check if this AMO was blocking other operations
                    if load in pending_dependencies.keys():
                        self._resolve_pending_dependencies(
                            load, pending_dependencies, is_rv64, has_fpud
                        )
            else:
                # AMO reading from initial value (0)
                data = self._get_data(load, None, is_rv64, has_fpud)
                load.compute_store_data(data, is_rv64)
                # Check if this AMO was blocking other operations
                if load in pending_dependencies:
                    self._resolve_pending_dependencies(
                        load, pending_dependencies, is_rv64, has_fpud
                    )

        assert pending_dependencies == {}, pending_dependencies

        # Pass 2: Create PC to load return mapping
        for load, prev_store in ordering.items():
            addr = load.exec_addr + SPIKE_STARTADDR
            data = self._get_data(load, prev_store, is_rv64, has_fpud)
            step_pc_to_load_return[addr] = data

        # Pass 3: Reset AMO data for next iteration
        for load, prev_store in ordering.items():
            if isinstance(load, Amo):
                load.reset_data()
            if isinstance(prev_store, Amo):
                prev_store.reset_data()

        return step_pc_to_load_return

    def _get_data(
        self,
        load: Load | Amo,
        prev_store: Optional[Store | Amo],
        is_rv64: bool,
        has_fpud: bool,
    ):
        """return the values that will be returned by a load, given its
        associated store operation
        """
        assert prev_store is not None
        size = load._memop_access_size()
        if prev_store is not None:
            data = prev_store.get_data()
            assert data is not None, f"store: {prev_store}"
            assert load.dest is not None
            assert prev_store.dest is not None
            offset = load.dest - prev_store.dest
            mask = (1 << (size * 8)) - 1
            data = (data >> (offset * 8)) & mask
            if load.instr_str in ("lb", "lh") or (is_rv64 and load.instr_str == "lw"):
                sign_bit = 8 * size - 1
                if (data >> sign_bit) & 1:
                    if is_rv64:
                        data |= (-1 << (size * 8)) & 0xFFFF_FFFF_FFFF_FFFF
                    else:
                        data |= (-1 << (size * 8)) & 0xFFFF_FFFF
        else:
            data = 0
        # handle NaN boxing
        if has_fpud and load.instr_str == "flw":
            data |= (-1 << (size * 8)) & 0xFFFF_FFFF_FFFF_FFFF
        return data

    def _resolve_pending_dependencies(
        self,
        resolved_store: Store | Amo,
        pending_dependencies: dict[Store | Amo, list[Load | Amo]],
        is_rv64: bool,
        has_fpud: bool,
    ):
        """Helper method to resolve dependencies when a store gets its data"""
        assert resolved_store in pending_dependencies.keys()
        waiting_loads = pending_dependencies[resolved_store]
        for waiting_load in waiting_loads:
            data = self._get_data(waiting_load, resolved_store, is_rv64, has_fpud)
            if isinstance(waiting_load, Amo):
                waiting_load.compute_store_data(data, is_rv64)
                # Recursively check if this newly resolved AMO was blocking others
                if waiting_load in pending_dependencies.keys():
                    self._resolve_pending_dependencies(
                        waiting_load, pending_dependencies, is_rv64, has_fpud
                    )
        del pending_dependencies[resolved_store]

    def __repr__(self) -> str:
        ret = ""
        for hartid, memops in self.step_memops.items():
            ret += f"HART{hartid}:\n"
            for memop in memops.values():
                ret += f"{memop.__repr__()}\n"
        return ret
