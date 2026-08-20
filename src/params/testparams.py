# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


import runparams
from params.pickparams import (
    ISAINSTRCLASS_INITIAL_BOOSTERS,
    EXCEPTION_OP_TYPE_INITIAL_BOOSTERS,
)
from params.get_designparams import *
from instgen.helper import ISAInstrClass
from riscv import ExceptionCause
from functools import cache
from typing import Optional, Dict
from random import Random
from enum import Enum, auto
from time import time_ns
import subprocess


class SimulatorEnum(Enum):
    CHIPYARD = auto()
    CHESHIRE = auto()
    XIANGSHAN = auto()
    TOOOBA = auto()
    NAXRISCV = auto()
    VEXIIRISCV = auto()


@cache
def _calibrate_spikespeed():
    """Runs a spike instance and returns the average nanoseconds per instruction"""
    NUM_INSTR = 10000
    script_dir = os.path.dirname(os.path.realpath(__file__))
    path_to_debug_file = os.path.join(script_dir, "_spike_calibration/calibration_cmds")
    path_to_elf = os.path.join(script_dir, "_spike_calibration/calibrate.elf")

    # Run the Spike command
    spike_shell_command = (
        "spike",
        "-d",
        f"--debug-cmd={path_to_debug_file}",
        f"--isa={'rv64g'}",
        path_to_elf,
    )

    ns_before = time_ns()
    subprocess.run(spike_shell_command, capture_output=True)
    ns_elapsed = time_ns() - ns_before

    return ns_elapsed / NUM_INSTR


def get_spike_timeout_seconds(spike_ns_per_instr) -> float:
    """
    return the average spike timout parameter, we add some slack to
    account for inaccuracies in busy systems
    """
    spike_slack_factor = 1000
    return max((spike_slack_factor * spike_ns_per_instr) / 1e9, 10)


class TestParams:
    # @param randseed for identification purposes only.
    def __init__(
        self,
        design_name: str,
        memsize: int,
        randseed: int,
        nmax_bbs: int,
        authorize_privileges: bool,
        local_random: Random,
        nmax_instrs: Optional[int] = None,
        custom_boosters = None
    ):
        self.prng = local_random

        # Program Identification Parameters
        self.randseed: int = randseed
        self.nmax_bbs: int = nmax_bbs
        self.nmax_instrs: Optional[int] = nmax_instrs
        self.memsize: int = memsize
        self.authorize_privileges: bool = authorize_privileges

        spike_ns_per_instr = _calibrate_spikespeed()
        self.spike_timeout: float = get_spike_timeout_seconds(spike_ns_per_instr)
        self.medeleg_mask: int = get_design_medelg_mask(design_name)
        self.mideleg_mask: int = get_design_midelg_mask(design_name)

        # DUT Parameters, FIXME HETERO, each heterogenous core will need one
        self.design_name: str = design_name
        self.boot_addr: int = get_design_boot_addr(design_name)
        # TODO all the extention below could be in the extention list
        self.is_rv64: bool = not is_design_32bit(design_name)
        self.c_ext_support: bool = c_ext_support(design_name)
        self.design_has_fpu: bool = f_ext_support(design_name)
        self.design_has_fpud: bool = d_ext_support(design_name)
        self.design_has_muldiv: bool = m_ext_support(design_name)
        self.design_has_amo: bool = a_ext_support(design_name)
        self.misaligned_data_support: bool = misaligned_data_support(design_name)
        self.s_mode_support: bool = s_mode_support(design_name)
        self.u_mode_support: bool = u_mode_support(design_name)
        self.pmp_support: bool = pmp_support(design_name)
        self.hartids: list[int] = get_design_hartids(design_name)
        self.num_harts: int = len(self.hartids)
        self.wfi_u_mode_avail: bool = wfi_u_mode_avail(design_name)
        self.clint_addr: int = get_design_clint_addr(design_name)
        self.design_extentions: list[str] = get_design_extentions(design_name)

        simulator_str = get_simulator(design_name)
        match simulator_str:
            case "chipyard":
                self.simulator = SimulatorEnum.CHIPYARD
            case "cheshire":
                self.simulator = SimulatorEnum.CHESHIRE
            case "xiangshan":
                self.simulator = SimulatorEnum.XIANGSHAN
            case "toooba":
                self.simulator = SimulatorEnum.TOOOBA
            case "naxriscv":
                self.simulator = SimulatorEnum.NAXRISCV
            case "vexiiriscv":
                self.simulator = SimulatorEnum.VEXIIRISCV
            case _:
                raise ValueError("Need a simulator")

        # Fuzzer State Initialization
        if self.design_has_fpu:
            self.proba_keep_fpu_state = self.prng.random() * 0.1

        self.isapickweights: dict[ISAInstrClass, float] = {}
        self.exceptionoppickweights: dict[ExceptionCause, float] = {}
        self.gen_pick_weights(custom_boosters)

        assert nmax_instrs is None or nmax_instrs > 0, (
            f"self.nmax_instrs ({nmax_instrs}) <= 0"
        )
        assert self.design_has_fpu or not self.design_has_fpud, (
            "Cannot have double but not simple precision"
        )

    def gen_pick_weights(self, custom_boosters: Optional[Dict[ISAInstrClass, float]] = None):
        """
        Generates the weights for the current generation instance. The weights
        define the probability of picking a certain instruction class or
        exception operation type.

        we use self.prng.uniform for instructions and exceptions to avoid removing
        a class that we want to test
        """

        def disable_conflicting_ops(isaclass: ISAInstrClass):
            """When fuzzing the memory consistency model, we must disable some
            operations.
            The interrupt fuzzing stuf conflict due to the deadlock risc posed
            by the 2 barriers
            The other are fuzzed in an attempt to remove some useless noise
            """
            match isaclass:
                # core that do not log the FPU comits values
                case ISAInstrClass.MEMFPU:
                    if (
                        "rocket" in self.design_name.lower()
                        or "naxriscv" in self.design_name.lower()
                        or "vexiiriscv" in self.design_name.lower()
                        or "boom" in self.design_name.lower()
                    ):
                        return 0
                case ISAInstrClass.MEMFPUD:
                    if (
                        "rocket" in self.design_name.lower()
                        or "naxriscv" in self.design_name.lower()
                        or "vexiiriscv" in self.design_name.lower()
                        or "boom" in self.design_name.lower()
                    ):
                        return 0
            return booster

        def disable_interrupts(isaclass: ISAInstrClass):
            match isaclass:
                case ISAInstrClass.WFI_NOTRAP:
                    return 0
                case ISAInstrClass.WFI_TRAP:
                    return 0
                case ISAInstrClass.SEND_IPI:
                    return 0
                case ISAInstrClass.SEND_LOCAL_INTERRUPT:
                    return 0
                case ISAInstrClass.CLEAR_INTERRUPT:
                    return 0
            return booster

        def disable_jit_scenario(isaclass: ISAInstrClass):
            match isaclass:
                case ISAInstrClass.WRITE_INSTR:
                    return 0
                case ISAInstrClass.WAIT_FOR_INSTR:
                    return 0
            return booster

        def disable_unsuported_dat_instrs(isaclass: ISAInstrClass):
            match isaclass:
                case ISAInstrClass.AMO:
                    return 0
                case ISAInstrClass.AMO64:
                    return 0
                case ISAInstrClass.MEM64:
                    return 0
            return booster

        # Can decrease the overall FPU load to favor other types of instructions
        if "toooba" in self.design_name:
            self.fpuweight = 0
        else:
            self.fpuweight = self.prng.uniform(0.05, 1)
        boosters = custom_boosters if custom_boosters is not None else ISAINSTRCLASS_INITIAL_BOOSTERS
        for isaclass in ISAInstrClass:
            booster = boosters[isaclass]
            if runparams.FUZZ_MCM:
                booster = disable_conflicting_ops(isaclass)
            if not runparams.FUZZ_INTERRUPT:
                booster = disable_interrupts(isaclass)
            if not runparams.JIT_SCENARIO:
                booster = disable_jit_scenario(isaclass)
            if runparams.VERIFY_MEMORY_TRACE == "dat":
                booster = disable_unsuported_dat_instrs(isaclass)
            weight = self.prng.uniform(0.05, 1) * booster
            if isaclass in [
                ISAInstrClass.MEMFPU,
                ISAInstrClass.FPU,
                ISAInstrClass.FPU64,
                ISAInstrClass.MEMFPUD,
                ISAInstrClass.FPUD,
                ISAInstrClass.FPUD64,
            ]:
                weight *= self.fpuweight
            self.isapickweights[isaclass] = weight

        for cause in ExceptionCause:
            self.exceptionoppickweights[cause] = (
                self.prng.uniform(0.05, 1) * EXCEPTION_OP_TYPE_INITIAL_BOOSTERS[cause]
            )

        if self.design_has_fpu:
            # Probability to change rounding mode instead of turning the FPU off
            self.proba_change_rm = self.prng.random()
        self.proba_ebreak_instead_of_ecall = self.prng.random()

        # Registers' initial values
        self.proba_reg_starts_with_zero = self.prng.random() / 10

        if __debug__:
            assert self.proba_reg_starts_with_zero >= 0.0
            assert self.proba_reg_starts_with_zero <= 1.0

    def set_nmax_instructions(self, nmax_instrs: int):
        self.nmax_instrs = nmax_instrs

    def instance_to_str(self) -> str:
        """return a string identifier of the current program"""
        return "{}_{}_{}_{}_{}".format(
            self.memsize,
            self.design_name,
            self.randseed,
            self.nmax_bbs,
            self.authorize_privileges,
        )
