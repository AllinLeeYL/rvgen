# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only
import string
import os
import subprocess

from mcm.types import *
from mcm.memops import *
from riscv import IntReg, FenceOrdering, Ordering
import runparams


class DartagnanSolve:
    """This generate a litmus test that is equivalent to the current test case,
    but which contains only the memoty informations.
    We use the following assumptions:
    - All PPO edges are equivalent, the cause does not matter. We can replace
    all syntactic dependencies with syntetic address dependencies
    - li will kill any dependency carried by the destination regioster
    - all PPO rules except the syntactic dependencies are fully defined by the
    memory operations alone
    """

    def __init__(
        self,
        step_memops: dict[HartId, dict[InstrCoord, MemInstr]],
        step_fences: dict[HartId, dict[InstrCoord, MemFence]],
        addr_regs: dict[HartId, dict[int, tuple[IntReg, int]]],
        outcome: dict[Addr, int],
        step_idx: int,
        test_id: str,
    ):
        self.step_idx: int = step_idx

        # output
        self.litmus_out: list[str] = ["RISCV hartattack"]
        self.litmus_file: str = os.path.join(
            runparams.PATH_TO_TMP, "litmus", f"step{step_idx}_{test_id}.litmus"
        )

        # data
        self.addr_regs_per_hart = addr_regs
        self.step_memops: dict[HartId, dict[InstrCoord, MemInstr]] = step_memops
        self.step_fences = step_fences
        self.next_litmus_reg: dict[HartId, int] = {}
        self.addr_to_reg: dict[HartId, dict[int, str]] = {}
        # exist condition, store as (hart, litmus-reg, value)
        self.exist_cond: list[tuple[HartId, str, int]] = []

        # syntactic deps, stored as dest idx, src regs
        self.syntactic_deps: dict[HartId, dict[PoIdx, set[str]]] = {}

        # initialize some stuff
        for hartid in addr_regs.keys():
            self.next_litmus_reg[hartid] = 1
            self.addr_to_reg[hartid] = {}
            self.syntactic_deps[hartid] = {}

        # get outcome
        self.step_outcome: dict[Addr, int] = {
            memop.exec_addr: outcome[memop.exec_addr + 0x80000000]
            for memops in self.step_memops.values()
            for memop in memops.values()
            if not isinstance(memop, Store)
        }

        self.data_reg = [self._get_next_reg(i) for i in range(len(addr_regs.keys()))]

    def gen_litmus(self):
        """Generates a litmus programs froma trace, which has the same
        properties as the original programs"""
        self._gen_initial_state()
        per_hart_instrs = self._gen_body_per_hart()
        self._concat_harts_body(per_hart_instrs)
        if not self.exist_cond:
            return
        self._gen_exist_condition()
        self._write_litmus()

    def run_litmus(self, step_str: str, backend: str = "herd"):
        """Runs the litmus file generated and checks for propepr execution"""
        if not self.exist_cond:
            return
        match backend:
            case "herd":
                self._run_litmus_on_herd(step_str)
            case "dat":
                self._run_litmus_on_dartagnan(step_str)
            case _:
                raise ValueError("backend not supported")

    def _run_litmus_on_dartagnan(self, step_str: str):
        dat3m_home = os.path.join(os.environ["DAT3M_HOME"])
        assert dat3m_home, "dat home not set"

        result = subprocess.run(
            [
                "java",
                "-jar",
                "dartagnan/target/dartagnan.jar",
                "--property=program_spec",
                "cat/riscv.cat",
                self.litmus_file,
            ],
            capture_output=True,
            text=True,
            cwd=dat3m_home,
            # timeout=60*10
        )
        # Check for Observation lines in stdout
        for line in result.stdout.splitlines():
            if line.startswith("Result"):
                if "FAIL" in line:
                    raise Exception(
                        f"Found MCM violation in step: {self.step_idx}\n{step_str}"
                    )
                if "PASS" in line:
                    return
        raise Exception(
            f"Somethig went wrong in: {self.step_idx}\n{step_str}"
        )

    def _run_litmus_on_herd(self, step_str: str):
        # Run herd7 command
        result = subprocess.run(
            [
                "herd7",
                "-model",
                "riscv.cat",
                "-speedcheck",
                "fast",
                "-timeout",
                "900",
                self.litmus_file,
            ],
            capture_output=True,
            text=True,
        )

        # Check if stdout is empty (indicates timeout)
        if not result.stdout.strip():
            print(result.stderr)
            raise Exception(f"Herd7 timeout: stdout is empty\n{step_str}")

        # Check for Observation lines in stdout
        for line in result.stdout.splitlines():
            if line.startswith("Observation"):
                if "Never" in line:
                    raise Exception(
                        f"Found MCM violation in step: {self.step_idx}\n{step_str}"
                    )

        # Delete litmus file if no exceptions were raised
        if os.path.exists(self.litmus_file):
            os.remove(self.litmus_file)

    def _write_litmus(self):
        with open(self.litmus_file, "w") as f:
            f.write("\n".join(self.litmus_out) + "\n")

    def _gen_initial_state(self):
        """Generates the address initialization section. All other register
        is initialized to 0

        We use one register per address, with no offset. Since there is an
        endless ammount of registers available in luitmus test, there is no need
        to use an offset
        """
        ret = ["{"]
        alphabet_iter = iter(string.ascii_lowercase)
        arch_addr_to_litmus_addr = {}
        for hartid, addr_regs in self.addr_regs_per_hart.items():
            hartregs = ""
            for addr, _ in addr_regs.items():
                if addr not in arch_addr_to_litmus_addr.keys():
                    litmus_addr = next(alphabet_iter)
                    while litmus_addr in ("d", "w", "i", "r"):
                        litmus_addr = next(alphabet_iter)
                    arch_addr_to_litmus_addr[addr] = litmus_addr
                else:
                    litmus_addr = arch_addr_to_litmus_addr[addr]
                litmus_reg = self._get_next_reg(hartid)
                hartregs += f"{hartid}:x{litmus_reg}={litmus_addr}; "
                self.addr_to_reg[hartid][addr] = f"x{litmus_reg}"
            ret.append(hartregs)
        ret += ["}"]
        # extend limus
        self.litmus_out.extend(ret)

    def _gen_body_per_hart(self) -> dict[HartId, list[str]]:
        """Generates the body of the litmus test.
        Iterates through the memeory operation and add the to the test.
        One instruction
        """
        # for each hart, make a list of litmus instructions
        hart_instrs_translated: dict[HartId, list[str]] = {}
        for hartid, hart_memops in self.step_memops.items():
            hart_instrs_translated[hartid] = []
            for memop in hart_memops.values():
                # get synctactic dependenies
                dependant_regs: set[str] = set()
                if memop.idx in self.syntactic_deps[memop.hartid].keys():
                    dependant_regs = self.syntactic_deps[memop.hartid][memop.idx]

                # add instruction
                if isinstance(memop, Load):
                    load_instrs = self._gen_load_str(memop, dependant_regs)
                    hart_instrs_translated[hartid].extend(load_instrs)
                elif isinstance(memop, Store):
                    store_instrs = self._gen_store_str(memop, dependant_regs)
                    hart_instrs_translated[hartid].extend(store_instrs)
                elif isinstance(memop, Amo):
                    amo_instrs = self._gen_amo_str(memop, dependant_regs)
                    hart_instrs_translated[hartid].extend(amo_instrs)
                else:
                    ValueError("Unknown memory operation type")

                # check if we are the predecessor of a fence and add it
                hart_fences = self.step_fences[hartid]
                for fence in hart_fences.values():
                    if memop.idx == fence.prev_memop_idx:
                        fence_str = self._fence_ordering_to_str(fence.fence_ordering)
                        hart_instrs_translated[hartid].append(fence_str)

        return hart_instrs_translated

    def _gen_exist_condition(self):
        """Generate the exists condition"""
        ret = ["exists"]
        cond = "("
        for idx, (hartid, reg, val) in enumerate(self.exist_cond):
            cond += f"{hartid}:{reg}={val}"
            if idx < len(self.exist_cond) - 1:
                cond += r" /\ "
        cond += ")"
        ret.append(cond)
        self.litmus_out.extend(ret)

    def _initiates_dependency(self, memop: Load | Amo, dest_reg: str) -> bool:
        # update syntactic deps
        initiates_dependency = False
        # adress dependencies
        if memop.syntactic_addr_dep:
            initiates_dependency = True
        # control dependencies
        if memop.syntactic_ctrl_dep:
            initiates_dependency = True
        # data dependencies
        if memop.syntactic_data_dep:
            initiates_dependency = True
        return initiates_dependency

    def _update_syntactic_dependencies(self, memop: Load | Amo) -> str:
        dep_carry_reg = f"x{self._get_next_reg(memop.hartid)}"
        # adress dependencies
        for dest in memop.syntactic_addr_dep:
            if dest not in self.syntactic_deps[memop.hartid]:
                self.syntactic_deps[memop.hartid][dest] = set()
            self.syntactic_deps[memop.hartid][dest].add(dep_carry_reg)

        # control dependencies
        for dest in memop.syntactic_ctrl_dep:
            if dest not in self.syntactic_deps[memop.hartid]:
                self.syntactic_deps[memop.hartid][dest] = set()
            self.syntactic_deps[memop.hartid][dest].add(dep_carry_reg)

        # data dependencies
        for dest in memop.syntactic_data_dep:
            if dest not in self.syntactic_deps[memop.hartid]:
                self.syntactic_deps[memop.hartid][dest] = set()
            self.syntactic_deps[memop.hartid][dest].add(dep_carry_reg)

        return dep_carry_reg

    def _gen_store_str(self, store: Store, dependant_regs: set[str]):
        """generate a load instruction: lw x7,0(x8) and updates the states
        Store do not initiate syntactic dependencies
        """
        assert store.dest
        addr_reg = self.addr_to_reg[store.hartid][store.dest]
        src_reg = f"x{self.data_reg[store.hartid]}"

        instrs = []
        # generate syntactic dependencies
        for reg in dependant_regs:
            instrs.append(f"or {addr_reg}, {addr_reg}, {reg}")
        instrs.append(f"li {src_reg}, {store.get_data()}")
        instrs.append(f"{store.instr_str} {src_reg}, 0({addr_reg})")
        return instrs

    def _gen_load_str(self, load: Load, dependant_regs: set[str]):
        """generate a load instruction: lw x7,0(x8) and updates the states"""
        assert load.dest
        addr_reg = self.addr_to_reg[load.hartid][load.dest]
        dest_reg = f"x{self._get_next_reg(load.hartid)}"

        instrs = []
        # generate syntactic dependencies
        for reg in dependant_regs:
            instrs.append(f"or {addr_reg}, {addr_reg}, {reg}")
        # gen load instruction
        instrs.append(f"{load.instr_str} {dest_reg}, 0({addr_reg})")
        # update exists cond
        mask = (1 << (load.mem_access_bytes * 8)) - 1
        self.exist_cond.append(
            (load.hartid, f"{dest_reg}", self.step_outcome[load.exec_addr] & mask)
        )
        # update syntactic deps
        initiates_dependency = self._initiates_dependency(load, dest_reg)
        if initiates_dependency:
            dep_carry_reg = self._update_syntactic_dependencies(load)
            instrs.append(f"xor {dep_carry_reg}, {dest_reg}, {dest_reg}")
        return instrs

    def _gen_amo_str(self, amo: Amo, dependant_regs: set[str]):
        """generate an amo instruction: amoswap.w.rl x1,x3,0(x9) and updates the
        states.
        Before the amo, we use li to make the source data
        """
        assert amo.dest
        addr_reg = self.addr_to_reg[amo.hartid][amo.dest]
        dest_reg = f"x{self._get_next_reg(amo.hartid)}"
        src_reg = f"x{self.data_reg[amo.hartid]}"
        instr_str = self._amo_ordering_to_str(amo)
        instrs = []
        # generate syntactic dependencies
        for reg in dependant_regs:
            instrs.append(f"or {addr_reg}, {addr_reg}, {reg}")
        instrs.append(f"li {src_reg}, {amo.get_data_rs2()}")
        instrs.append(f"{instr_str} {dest_reg}, {src_reg}, ({addr_reg})")
        # update exists cond
        mask = (1 << (amo.mem_access_bytes * 8)) - 1
        self.exist_cond.append(
            (amo.hartid, f"{dest_reg}", self.step_outcome[amo.exec_addr] & mask)
        )
        # update syntactic deps
        initiates_dependency = self._initiates_dependency(amo, dest_reg)
        if initiates_dependency:
            dep_carry_reg = self._update_syntactic_dependencies(amo)
            instrs.append(f"xor {dep_carry_reg}, {dest_reg}, {dest_reg}")
        return instrs

    def _concat_harts_body(self, instr_per_hart: dict[HartId, list[str]]):
        last_hartid = len(instr_per_hart.keys()) - 1

        # add the first line
        processor_line = ""
        for hartid in self.addr_to_reg.keys():
            if hartid == last_hartid:
                entry_terminator = "  ;"
            else:
                entry_terminator = "  |  "
            processor_line += f"P{hartid}{entry_terminator}"
        ret = [processor_line]

        # add body
        len_body = max(len(instrs) for instrs in instr_per_hart.values())
        for instr_idx in range(len_body):
            line = ""
            for hartid, hart_instrs in instr_per_hart.items():
                if hartid == last_hartid:
                    entry_terminator = "  ;"
                else:
                    entry_terminator = "  |  "
                if instr_idx >= len(hart_instrs):
                    # skip harts that are done
                    line += entry_terminator
                    continue
                instr = hart_instrs[instr_idx]
                line += f"{instr}{entry_terminator}"
            ret.append(line)

        # extend limus
        self.litmus_out.extend(ret)

    def _fence_ordering_to_str(self, ordering: FenceOrdering):
        match ordering:
            case FenceOrdering.RW_RW:
                return "fence rw,rw"
            case FenceOrdering.TSO:
                return "fence.tso"
            case FenceOrdering.RW_W:
                return "fence rw,w"
            case FenceOrdering.R_RW:
                return "fence r,rw"
            case FenceOrdering.R_R:
                return "fence r,r"
            case FenceOrdering.W_W:
                return "fence w,w"
            case _:
                raise ValueError("unknown fence ordering")

    def _amo_ordering_to_str(self, amo: Amo):
        match amo.ordering:
            case Ordering.NoOrd:
                return f"{amo.instr_str}"
            case Ordering.Rel:
                return f"{amo.instr_str}.rl"
            case Ordering.Acq:
                return f"{amo.instr_str}.aq"
            case Ordering.Seq:
                return f"{amo.instr_str}.aq.rl"
            case _:
                ValueError("unknown ordering")

    def _get_next_reg(self, hartid: int):
        reg = self.next_litmus_reg[hartid]
        self.next_litmus_reg[hartid] += 1
        return reg
