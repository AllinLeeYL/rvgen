# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


from mcm.types import Addr, HartId, InstrCoord, PoIdx
from riscv import FenceOrdering, Ordering
from typing import Optional


class MemInstr:
    """tracks the informations relevant for memory instructions"""

    def __init__(
        self,
        instr_str: str,
        exec_addr: Addr,
        idx: PoIdx,
        instr_coord: InstrCoord,
        dest_addr: Optional[Addr],
        hartid: HartId,
        step: int,
    ):
        # Instruction information
        self.hartid: HartId = hartid
        self.instr_str: str = instr_str
        self.exec_addr: Addr = exec_addr
        self.idx: PoIdx = idx
        self.step: int = step
        self.instr_coord: InstrCoord = instr_coord
        self.dest: Optional[Addr] = dest_addr
        self.mem_access_bytes = self._memop_access_size()
        # Constaraints
        self.rw_w_overlap: set[PoIdx] = set()
        self.r_r_overlap: set[PoIdx] = set()
        self.amo_read: set[PoIdx] = set()
        self.fence_successor: set[PoIdx] = set()
        self.acquire: set[PoIdx] = set()
        self.release: set[PoIdx] = set()
        self.sequential: set[PoIdx] = set()
        self.syntactic_addr_dep: set[PoIdx] = set()
        self.syntactic_data_dep: set[PoIdx] = set()
        self.syntactic_ctrl_dep: set[PoIdx] = set()
        self.rule_12: set[PoIdx] = set()
        self.rule_13: set[PoIdx] = set()

    def add_dest_addr(self, dest_addr: int):
        self.dest = dest_addr

    def _memop_access_size(self):
        if self.instr_str in ("sb", "lb", "lbu"):
            ret = 1
        elif self.instr_str in ("sh", "lh", "lhu"):
            ret = 2
        elif (
            self.instr_str in ("sw", "lw", "lwu", "flw", "fsw")
            or ".w" in self.instr_str
        ):
            ret = 4
        elif self.instr_str in ("sd", "ld", "fld", "fsd") or ".d" in self.instr_str:
            ret = 8
        else:
            raise RuntimeError("Unexpected instruction string")
        return ret

    def add_data(self, data: int):
        raise NotImplementedError("Likely tried to add data to a load")

    def _check_overlap(self, instr_b: "MemInstr") -> bool:
        """checks if 2 instructions destination address overlap"""
        assert self.dest is not None and instr_b.dest is not None
        instr_a_range = range(self.dest, self.dest + self.mem_access_bytes)
        instr_b_range = range(instr_b.dest, instr_b.dest + instr_b.mem_access_bytes)
        if (
            instr_a_range.start < instr_b_range.stop
            and instr_a_range.stop > instr_b_range.start
        ):
            return True
        return False

    def get_dest(self):
        assert self.dest is not None
        return self.dest

    def __repr__(self) -> str:
        dest = 0 if self.dest is None else self.dest
        ret = f"{self.instr_str:<4} exec @{hex(self.exec_addr + 0x80000000)} dest: {hex(dest + 0x80000000)} "
        if self.rw_w_overlap:
            ret += f"rw-w: {str(self.rw_w_overlap)} "
        if self.r_r_overlap:
            ret += f"r-r: {self.r_r_overlap} "
        if self.fence_successor:
            ret += f"fence succ: ({str(self.fence_successor)}) "
        if self.acquire:
            ret += f"acquire: {str(self.acquire)} "
        if self.release:
            ret += f"release: {str(self.release)} "
        if self.sequential:
            ret += f"sequential: {str(self.sequential)} "
        if self.syntactic_data_dep:
            ret += f"data: {str(self.syntactic_data_dep)} "
        if self.syntactic_ctrl_dep:
            ret += f"ctrl: {str(self.syntactic_ctrl_dep)} "
        if self.syntactic_addr_dep:
            ret += f"addr: {str(self.syntactic_addr_dep)} "
        if self.rule_12:
            ret += f"pipileine 1: {str(self.rule_12)} "
        if self.rule_13:
            ret += f"pipileine 2: {str(self.rule_13)} "
        return ret


class Amo(MemInstr):
    """wrapper type to differenciate load and stores"""

    def __init__(
        self,
        instr_str: str,
        exec_addr: Addr,
        idx: PoIdx,
        instr_coord: InstrCoord,
        dest_addr: Optional[Addr],
        hartid: HartId,
        step: int,
        ordering: Ordering,
    ):
        super().__init__(
            instr_str, exec_addr, idx, instr_coord, dest_addr, hartid, step
        )
        self.ordering = ordering
        self._data = None

    def add_data_rs2(self, data: int):
        """adds the data used in the "compare" field"""
        size = self._memop_access_size()
        mask = (1 << (size * 8)) - 1
        self.data_rs2 = data & mask

    def compute_store_data(self, prev_store_data: int, is_rv64: bool):
        """Given the data of the associated store operation, computes the data
        that will be store with respect to the rs2 value and operation

        after parsing, all values are interpreted as unsigned, so we sign the
        values if needed by the operation

        WARNING RV32 not tested for atomics
        """

        def to_signed(a: int) -> int:
            if is_rv64:
                return a if a < (1 << 63) else a - (1 << 64)
            else:
                return a if a < (1 << 31) else a - (1 << 32)

        if "amoswap" in self.instr_str:
            self._data = self.data_rs2
        elif "amoadd" in self.instr_str:
            self._data = prev_store_data + self.data_rs2
        elif "amoxor" in self.instr_str:
            self._data = prev_store_data ^ self.data_rs2
        elif "amoand" in self.instr_str:
            self._data = prev_store_data & self.data_rs2
        elif "amoor" in self.instr_str:
            self._data = prev_store_data | self.data_rs2
        elif "amominu" in self.instr_str:
            self._data = min(prev_store_data, self.data_rs2)
        elif "amomaxu" in self.instr_str:
            self._data = max(prev_store_data, self.data_rs2)
        elif "amomin" in self.instr_str:
            data_rs2_signed = to_signed(self.data_rs2)
            prev_store_data_signed = to_signed(prev_store_data)
            self._data = min(prev_store_data_signed, data_rs2_signed)
        elif "amomax" in self.instr_str:
            data_rs2_signed = to_signed(self.data_rs2)
            prev_store_data_signed = to_signed(prev_store_data)
            self._data = max(prev_store_data_signed, data_rs2_signed)

        # FIXME, those should have their own types !!, the LrSc type
        elif "lr" in self.instr_str:
            raise ValueError("LR not yet supported")
        elif "sc" in self.instr_str:
            raise ValueError("SC not yet supported")
            return self.data_rs2
        else:
            raise ValueError("Unexpected Instruction String")

    def get_data(self):
        data = self._data
        return data

    def get_data_rs2(self):
        data = self.data_rs2
        return data

    def reset_data(self):
        self._data = None


def make_syntethic_init_store(addr: int, init_idx: int):
    """creates a synthetic store for init events. We use -1 as they happen on
    all harts, and this value can be used to differentiate
    """
    store = Store(
        "sd",
        dest_addr=addr,
        exec_addr=-1,
        idx=init_idx,
        instr_coord=(-1, -1),
        hartid=-1,
        step=-1,
    )
    # add default data
    store.add_data(0)
    return store


class Store(MemInstr):
    """wrapper type to differenciate load and stores"""

    def __init__(
        self,
        instr_str: str,
        exec_addr: Addr,
        idx: PoIdx,
        instr_coord: InstrCoord,
        dest_addr: Optional[Addr],
        hartid: HartId,
        step: int,
    ):
        super().__init__(
            instr_str, exec_addr, idx, instr_coord, dest_addr, hartid, step
        )
        self._data = None

    def add_data(self, data: int):
        """adds the data to be stored
        data is the raw value stored in the register, gathered during the
        pre-isa simulation. We adapt the size of the data to the size of the
        store
        """
        size = self._memop_access_size()
        mask = (1 << (size * 8)) - 1
        self._data = data & mask

    def get_data(self):
        assert self._data is not None, "did you run the pre-ISA simulation?"
        return self._data


class Load(MemInstr):
    """wrapper type to differenciate load and stores"""

    def __init__(
        self,
        instr_str: str,
        exec_addr: Addr,
        idx: PoIdx,
        instr_coord: InstrCoord,
        dest_addr: Optional[Addr],
        hartid: HartId,
        step: int,
    ):
        super().__init__(
            instr_str, exec_addr, idx, instr_coord, dest_addr, hartid, step
        )


class MemFence:
    """Tracks the informations relevant for fence instructions"""

    def __init__(
        self,
        exec_addr: Addr,
        fence_ordering: FenceOrdering,
        prev_memop_idx: int,
        instr_coord: InstrCoord,
        step: int,
    ):
        self.instr_str: str = "fence"
        self.exec_addr: Addr = exec_addr
        self.prev_memop_idx: PoIdx = prev_memop_idx
        self.instr_coord: InstrCoord = instr_coord
        self.fence_ordering: FenceOrdering = fence_ordering
        self.predecessor_set: list[PoIdx] = []
        self.successor_set: list[PoIdx] = []
        self.step: int = step

    def __repr__(self) -> str:
        ret = (
            f"fence {self.fence_ordering}, "
            f"exec@ {hex(self.exec_addr)}, "
            f"prev idx: {self.prev_memop_idx}"
        )
        return ret
