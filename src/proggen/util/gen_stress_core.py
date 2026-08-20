# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only
from enum import Enum, auto
from typing import TYPE_CHECKING
from instgen import (
    IntLoadInstr,
    IntStoreInstr,
    FloatLoadInstr,
    FloatStoreInstr,
    R12DInstr,
    ImmRdInstr,
    RegImmInstr,
    RVInstr,
    JALRInstr,
)
from riscv import IntReg, FloatReg
from asm import li_into_reg
from params import RELOCATOR_REG, STRESS_SECTION_BASE, ILEN

if TYPE_CHECKING:
    from states import FuzzerState
    from params import TestParams


class LoadAddrType(Enum):
    RandomAddr = auto()
    OverlappingAddr = auto()


class StressCoreGen:
    """class responsible for the generation of a stress inducing
    program, meant to run on an isolated core. The aim is to trigger
    out of order behaviour on other cores, so overlapping access are
    prioritized
    """

    addr_regs = [
        IntReg.a0,
        IntReg.a1,
        IntReg.a2,
        IntReg.a3,
        IntReg.a4,
        IntReg.a5,
        IntReg.a6,
        IntReg.a7,
    ]
    ret_regs = [
        IntReg.s0,
        IntReg.s1,
        IntReg.s2,
        IntReg.s3,
        IntReg.s4,
        IntReg.s5,
        IntReg.s6,
        IntReg.s7,
        IntReg.s8,
        IntReg.s9,
        IntReg.s10,
        IntReg.s11,
    ]

    def __init__(self, test_params: "TestParams", fuzzerstate: "FuzzerState"):
        # start with largest, unused
        self.curr_load_alignment: int = 3  # max 3
        self.curr_store_alignment: int = 3  # max 3
        self.curr_load_addr_reg: IntReg = IntReg.a0
        self.curr_store_addr_reg: IntReg = IntReg.a1
        self.stress_code: list[RVInstr] = []
        self.prev_producer_idx: int = 0

        # Initialize the first addresses
        next_load_loc = self._gen_new_load_addr(fuzzerstate)
        self.curr_load_addr_reg = self._gen_addr_reg(
            test_params, fuzzerstate, next_load_loc
        )
        self.curr_store_addr_range: int = 0
        next_store_addr = self._gen_new_store_addr(fuzzerstate)
        self.curr_store_addr_reg = self._gen_addr_reg(
            test_params, fuzzerstate, next_store_addr
        )

    def _get_addr_alignment(self, addr: int):
        """Returns the number of alignment bits"""
        # isolate the LSB set to 1
        return (addr & -addr).bit_length() - 1

    def _select_load_op(self, test_params: "TestParams"):
        "selects a load instruction to perform"
        load_ops = ["lb", "lbu", "lh", "lhu", "lw", "lwu", "flw", "ld", "fld"]
        weights = [1] * len(load_ops)
        if not test_params.is_rv64:
            weights[load_ops.index("ld")] = 0
            weights[load_ops.index("lwu")] = 0
        if not test_params.design_has_fpu:
            weights[load_ops.index("flw")] = 0
        if not test_params.design_has_fpud:
            weights[load_ops.index("fld")] = 0
        if self.curr_load_alignment < 1:
            weights[load_ops.index("lh")] = 0
            weights[load_ops.index("lhu")] = 0
        if self.curr_load_alignment < 2:
            weights[load_ops.index("lw")] = 0
            weights[load_ops.index("lwu")] = 0
            weights[load_ops.index("flw")] = 0
        if self.curr_load_alignment < 3:
            weights[load_ops.index("ld")] = 0
            weights[load_ops.index("fld")] = 0
        return test_params.prng.choices(load_ops, weights, k=1)[0]

    def _select_store_op(self, test_params: "TestParams"):
        store_ops = ["sb", "sh", "sw", "fsw", "sd", "fsd"]
        weights = [1] * len(store_ops)
        if not test_params.is_rv64:
            weights[store_ops.index("sd")] = 0
        if not test_params.design_has_fpu:
            weights[store_ops.index("fsw")] = 0
        if not test_params.design_has_fpud:
            weights[store_ops.index("fsd")] = 0
        if self.curr_store_alignment < 1:
            weights[store_ops.index("sh")] = 0
        if self.curr_store_alignment < 2:
            weights[store_ops.index("sw")] = 0
            weights[store_ops.index("fsw")] = 0
        if self.curr_store_alignment < 3:
            weights[store_ops.index("sd")] = 0
            weights[store_ops.index("fsd")] = 0
        return test_params.prng.choices(store_ops, weights, k=1)[0]

    def _gen_new_store_addr(self, fuzzerstate: "FuzzerState"):
        """Selects the next store address. The address must be unsued, as to not
        interfere with the testcase
        """
        # Random alignment and doubleword (TODO maybe different sizes ?)
        next_store_addr = fuzzerstate.memstate.gen_random_free_addr(
            0, 1 << 3, 0, fuzzerstate.memstate.memsize
        )
        if next_store_addr is None:
            raise ValueError("Out Of Memory")
        self.curr_store_addr_range = fuzzerstate.memstate.get_available_contig_space(
            next_store_addr
        )
        self.curr_store_alignment = self._get_addr_alignment(next_store_addr)
        assert next_store_addr not in fuzzerstate.store_locations
        return next_store_addr

    def _gen_new_load_addr(self, fuzzerstate: "FuzzerState"):
        """Generate a load address. Addresses overlapping with the other cores
        are prioritized.
        We can choose any address with no side effect
        """
        # Bias towards OverlappingAddr
        addr_type = fuzzerstate.prng.choices(
            population=list(LoadAddrType),
            weights=[0.2, 0.8],  # 20% RandomAddr, 80% OverlappingAddr
            k=1,
        )[0]
        match addr_type:
            case LoadAddrType.RandomAddr:
                next_load_loc = fuzzerstate.prng.randrange(
                    0, fuzzerstate.memstate.memsize
                )
            case LoadAddrType.OverlappingAddr:
                next_load_loc = fuzzerstate.prng.choice(fuzzerstate.store_locations)

        self.curr_load_alignment = self._get_addr_alignment(next_load_loc)
        return next_load_loc

    def _gen_addr_reg(
        self, test_params: "TestParams", fuzzerstate: "FuzzerState", addr: int
    ):
        assert addr < 0x80000000
        reg_weights = [1] * len(self.addr_regs)
        reg_weights[self.addr_regs.index(self.curr_store_addr_reg)] = 0
        reg_weights[self.addr_regs.index(self.curr_load_addr_reg)] = 0
        addr_reg = fuzzerstate.prng.choices(
            population=self.addr_regs, weights=reg_weights, k=1
        )[0]
        lui_imm, addi_imm = li_into_reg(addr)
        self.stress_code += [
            ImmRdInstr("lui", addr_reg, lui_imm, test_params.is_rv64),
            RegImmInstr("addi", addr_reg, addr_reg, addi_imm, test_params.is_rv64),
            R12DInstr("add", addr_reg, addr_reg, RELOCATOR_REG[0]),
        ]
        self.prev_producer_idx = len(self.stress_code)
        return addr_reg

    def _gen_load_instr(self, test_params: "TestParams"):
        instr = self._select_load_op(test_params)
        if "f" in instr:
            frd = test_params.prng.choice(list(FloatReg))
            return FloatLoadInstr(
                instr, frd, self.curr_load_addr_reg, 0, None, test_params.is_rv64
            )
        else:
            rd = test_params.prng.choice(self.ret_regs)
            return IntLoadInstr(
                instr, rd, self.curr_load_addr_reg, 0, None, test_params.is_rv64
            )

    def _gen_store_instr(self, test_params: "TestParams"):
        instr = self._select_store_op(test_params)
        if "f" in instr:
            frd = test_params.prng.choice(list(FloatReg))
            return FloatStoreInstr(
                instr, self.curr_store_addr_reg, frd, 0, None, test_params.is_rv64
            )
        else:
            rd = test_params.prng.choice(self.ret_regs)
            return IntStoreInstr(
                instr, self.curr_store_addr_reg, rd, 0, None, test_params.is_rv64
            )

    def gen_stress_core(self, fuzzerstate: "FuzzerState", test_params: "TestParams"):
        """Generates the loop. Each iteration, a new pattern can be selected
        We generate a stress sequence of at most one page, which loops
        indefinetly
        """
        lui_imm = (STRESS_SECTION_BASE - RELOCATOR_REG[1]) >> 12
        # FIXME magic num, 7 is the worst case scenario (1 memop + 3 prod) plus the extra 3 loop instructions
        while len(self.stress_code) + 7 < 0x1000 / ILEN:
            curr_rand = test_params.prng.randrange(0, 20)
            # Gen New Addr
            if curr_rand < 4:
                new_load_addr = test_params.prng.choice([True, False])
                if new_load_addr:
                    next_load_loc = self._gen_new_load_addr(fuzzerstate)
                    self.curr_load_addr_reg: IntReg = self._gen_addr_reg(
                        test_params, fuzzerstate, next_load_loc
                    )
                else:
                    next_store_addr = self._gen_new_store_addr(fuzzerstate)
                    self.curr_store_addr_reg: IntReg = self._gen_addr_reg(
                        test_params, fuzzerstate, next_store_addr
                    )
                assert self.curr_load_addr_reg != self.curr_store_addr_reg

            # gen load
            if curr_rand < 10:
                self.stress_code.append(self._gen_load_instr(test_params))
            # gen_store
            else:
                self.stress_code.append(self._gen_store_instr(test_params))
            # Gen Loop, max backjmp to prev_producer_idx
            # TODO change offset
        # Loop back to the start of the stress code
        self.stress_code += [
            ImmRdInstr("lui", IntReg.t0, lui_imm, test_params.is_rv64),
            R12DInstr("add", IntReg.t0, IntReg.t0, RELOCATOR_REG[0]),
            JALRInstr("jalr", IntReg.zero, IntReg.t0, 0, None, test_params.is_rv64),
        ]
        assert len(self.stress_code) < 1024, len(self.stress_code)
        return self.stress_code
