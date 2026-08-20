# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


# This module picks some random valid CSR operation.

from riscv import CSR, IntReg, PrivLvl
from toleratebugs import (
    is_no_interaction_minstret,
    is_tolerate_kronos_minstret,
    is_tolerate_vexriscv_minstret,
    is_tolerate_picorv32_missingmandatorycsrs,
    is_tolerate_picorv32_readnonimplcsr,
    is_tolerate_picorv32_writehpm,
    is_tolerate_cva6_mhpmcounter,
    is_tolerate_boom_minstret,
    is_tolerate_picorv32_readhpm_nocsrrs,
    is_tolerate_vexriscv_mhpmcountern,
    is_tolerate_cva6_mhpmevent31,
)
from instgen import (
    CSRRegInstr,
    CSRImmInstr,
)
from enum import Enum, auto
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from states import CoreState
    from params import TestParams


class MachineCsrCandidates64(Enum):
    SCAUSE = auto()
    MCAUSE = auto()
    SSCRATCH = auto()
    MSCRATCH = auto()
    MINSTRET = auto()
    MHPMCOUNTER3 = auto()
    MHPMEVENT31 = auto()
    MIE = auto()
    SIE = auto()


class MachineCsrCandidates32(Enum):
    SCAUSE = auto()
    MCAUSE = auto()
    SSCRATCH = auto()
    MSCRATCH = auto()
    MINSTRET = auto()
    MIE = auto()
    SIE = auto()


# FIXME add interrupt registers, most interesting are SIE, SIP, SSTATUS


class SupervisorCsrCandidates(Enum):
    SCAUSE = auto()
    SSCRATCH = auto()
    SIE = auto()


def gen_random_csr_op(corestate: "CoreState", test_params: "TestParams"):
    """Randomly reads or write, if allowed, a CSR register"""
    if __debug__:
        assert corestate.hartstate.privlvl in (
            PrivLvl.Machine,
            PrivLvl.Supervisor,
        )

    if corestate.hartstate.privlvl == PrivLvl.Machine:
        if test_params.is_rv64:
            target_csr = None
            # Build candidate list with weights based on constraints
            candidates = list(MachineCsrCandidates64)
            weights = [1.0] * len(candidates)
            # Exclude MINSTRET if is_no_interaction_minstret
            if is_no_interaction_minstret(test_params.design_name):
                idx = candidates.index(MachineCsrCandidates64.MINSTRET)
                weights[idx] = 0.0
            # Exclude SCAUSE and SSCRATCH if no s_mode_support
            if not test_params.s_mode_support:
                for csr in (
                    MachineCsrCandidates64.SCAUSE,
                    MachineCsrCandidates64.SSCRATCH,
                ):
                    idx = candidates.index(csr)
                    weights[idx] = 0.0
            # Exclude MHPMCOUNTER3 and MHPMEVENT31 for cva6 if not tolerated
            if (
                "cva6" in test_params.design_name
                and not is_tolerate_cva6_mhpmcounter()
                and not is_tolerate_cva6_mhpmevent31()
            ):
                for csr in (
                    MachineCsrCandidates64.MHPMCOUNTER3,
                    MachineCsrCandidates64.MHPMEVENT31,
                ):
                    idx = candidates.index(csr)
                    weights[idx] = 0.0
            # Exclude SCAUSE and MCAUSE if it is an interrupt block
            if corestate.interrupt_block:
                for csr in (
                    MachineCsrCandidates64.MCAUSE,
                    MachineCsrCandidates64.SCAUSE,
                    MachineCsrCandidates64.SIE,
                    MachineCsrCandidates64.MIE,
                ):
                    idx = candidates.index(csr)
                    weights[idx] = 0.0

            target_csr = test_params.prng.choices(candidates, weights=weights, k=1)[0]

            if target_csr == MachineCsrCandidates64.SCAUSE:
                # According to the spec, the SCAUSE CSR must be able to hold bits 0 to 4. mret is not required to.
                ret = CSRImmInstr(
                    "csrrwi",
                    corestate.intregpickstate.pick_int_outputreg(),
                    test_params.prng.randrange(32),
                    CSR.SCAUSE,
                )
            elif target_csr == MachineCsrCandidates64.MCAUSE:
                ret = CSRImmInstr(
                    "csrrwi",
                    corestate.intregpickstate.pick_int_outputreg(),
                    test_params.prng.randrange(16),
                    CSR.MCAUSE,
                )
            elif target_csr == MachineCsrCandidates64.SSCRATCH:
                rs1 = corestate.intregpickstate.pick_int_inputreg()
                rd = corestate.intregpickstate.pick_int_outputreg()
                ret = CSRRegInstr("csrrw", rd, rs1, CSR.SSCRATCH)
            elif target_csr == MachineCsrCandidates64.MSCRATCH:
                rs1 = corestate.intregpickstate.pick_int_inputreg()
                rd = corestate.intregpickstate.pick_int_outputreg()
                ret = CSRRegInstr("csrrw", rd, rs1, CSR.MSCRATCH)
            elif target_csr == MachineCsrCandidates64.MINSTRET:
                if corestate.rocket_minsret_innacurate or (
                    "boom" in test_params.design_name
                    and not is_tolerate_boom_minstret()
                ):
                    ret = CSRImmInstr(
                        "csrrwi",
                        IntReg.zero,
                        test_params.prng.randrange(16),
                        CSR.MINSTRET,
                    )
                else:
                    rs1 = corestate.intregpickstate.pick_int_inputreg()
                    rd = corestate.intregpickstate.pick_int_outputreg()
                    ret = CSRRegInstr("csrrw", rd, rs1, CSR.MINSTRET)
                corestate.rocket_minsret_innacurate = False
            elif target_csr == MachineCsrCandidates64.MHPMCOUNTER3:
                ret = CSRImmInstr(
                    "csrrwi",
                    IntReg.zero,
                    test_params.prng.randrange(16),
                    CSR.MHPMCOUNTER3,
                )
            elif target_csr == MachineCsrCandidates64.MHPMEVENT31:
                ret = CSRImmInstr(
                    "csrrwi",
                    IntReg.zero,
                    test_params.prng.randrange(16),
                    CSR.MHPMEVENT31,
                )
            elif target_csr == MachineCsrCandidates64.SIE:
                rd = corestate.intregpickstate.pick_int_outputreg()
                ret = CSRRegInstr("csrrs", rd, IntReg.zero, CSR.SIE)
            elif target_csr == MachineCsrCandidates64.MIE:
                rd = corestate.intregpickstate.pick_int_outputreg()
                ret = CSRRegInstr("csrrs", rd, IntReg.zero, CSR.MIE)
            else:
                raise Exception("Unexpected target_csr: {}".format(target_csr))
        else:
            if (
                "vexriscv" in test_params.design_name
                and is_tolerate_vexriscv_mhpmcountern()
            ):
                rd = corestate.intregpickstate.pick_int_outputreg()
                return CSRImmInstr(
                    "csrrwi", rd, test_params.prng.randrange(16), CSR.MHPMCOUNTER3
                )
            elif (
                "picorv32" in test_params.design_name
                and not is_tolerate_picorv32_missingmandatorycsrs()
                and not is_tolerate_picorv32_readnonimplcsr()
            ):
                assert not is_no_interaction_minstret(test_params.design_name), (
                    "picorv32 only has minstret in this config."
                )
                target_csr = MachineCsrCandidates32.MINSTRET
            else:
                candidates = list(MachineCsrCandidates32)
                weights = [1.0] * len(candidates)
                # Exclude MINSTRET if is_no_interaction_minstret
                if is_no_interaction_minstret(test_params.design_name):
                    idx = candidates.index(MachineCsrCandidates32.MINSTRET)
                    weights[idx] = 0.0
                # Exclude SCAUSE and SSCRATCH if no s_mode_support
                if not test_params.s_mode_support:
                    for csr in (
                        MachineCsrCandidates32.SCAUSE,
                        MachineCsrCandidates32.SSCRATCH,
                    ):
                        idx = candidates.index(csr)
                        weights[idx] = 0.0
                # Exclude MIE, SIE, MCAUSE, SCAUSE if it is an interrupt block
                if corestate.interrupt_block:
                    for csr in (
                        MachineCsrCandidates32.MIE,
                        MachineCsrCandidates32.SIE,
                        MachineCsrCandidates32.MCAUSE,
                        MachineCsrCandidates32.SCAUSE,
                    ):
                        idx = candidates.index(csr)
                        weights[idx] = 0.0

                target_csr = test_params.prng.choices(candidates, weights=weights, k=1)[
                    0
                ]
            if target_csr == MachineCsrCandidates32.SCAUSE:
                # According to the spec, the SCAUSE CSR must be able to hold
                # bits 0 to 4. mret is not required to.
                if (
                    "vexriscv" in test_params.design_name
                ):  # vexriscv complies with the privileged spec v1.10, which
                    # does not require scause to hold the 5th bit. Similarly,
                    # kronos implements privileged spec v1.11
                    randval = test_params.prng.randrange(16)
                    rd = corestate.intregpickstate.pick_int_outputreg()
                    ret = CSRImmInstr("csrrwi", rd, randval, CSR.SCAUSE)
                else:
                    rd = corestate.intregpickstate.pick_int_outputreg()
                    ret = CSRImmInstr(
                        "csrrwi", rd, test_params.prng.randrange(32), CSR.SCAUSE
                    )
            elif target_csr == MachineCsrCandidates32.MCAUSE:
                rd = corestate.intregpickstate.pick_int_outputreg()
                ret = CSRImmInstr(
                    "csrrwi", rd, test_params.prng.randrange(16), CSR.MCAUSE
                )
            elif target_csr == MachineCsrCandidates32.SSCRATCH:
                rs1 = corestate.intregpickstate.pick_int_inputreg()
                rd = corestate.intregpickstate.pick_int_outputreg()
                ret = CSRRegInstr("csrrw", rd, rs1, CSR.SSCRATCH)
            elif target_csr == MachineCsrCandidates32.MSCRATCH:
                rs1 = corestate.intregpickstate.pick_int_inputreg()
                rd = corestate.intregpickstate.pick_int_outputreg()
                ret = CSRRegInstr("csrrw", rd, rs1, CSR.MSCRATCH)
            elif target_csr == MachineCsrCandidates32.MINSTRET:
                if (
                    "kronos" in test_params.design_name
                    and not is_tolerate_kronos_minstret()
                    or "vexriscv" in test_params.design_name
                    and not is_tolerate_vexriscv_minstret()
                ):
                    rs1 = corestate.intregpickstate.pick_int_inputreg()
                    ret = CSRRegInstr("csrrw", IntReg.zero, rs1, CSR.MINSTRET)
                elif "picorv32" in test_params.design_name:
                    if is_tolerate_picorv32_readhpm_nocsrrs():
                        opcode_str = test_params.prng.choice(("csrrw", "csrrs"))
                    else:
                        opcode_str = "csrrs"
                    if is_tolerate_picorv32_writehpm():
                        rs1 = corestate.intregpickstate.pick_int_inputreg()
                    else:
                        rs1 = IntReg.zero
                    rd = corestate.intregpickstate.pick_int_outputreg()
                    ret = CSRRegInstr(opcode_str, rd, rs1, CSR.MINSTRET)
                else:
                    rs1 = corestate.intregpickstate.pick_int_inputreg()
                    rd = corestate.intregpickstate.pick_int_outputreg()
                    ret = CSRRegInstr("csrrw", rd, rs1, CSR.MINSTRET)
            elif target_csr == MachineCsrCandidates32.SIE:
                rd = corestate.intregpickstate.pick_int_outputreg()
                ret = CSRRegInstr("csrrs", rd, IntReg.zero, CSR.SIE)
            elif target_csr == MachineCsrCandidates32.MIE:
                rd = corestate.intregpickstate.pick_int_outputreg()
                ret = CSRRegInstr("csrrs", rd, IntReg.zero, CSR.MIE)
            else:
                raise Exception("Unexpected target_csr: {}".format(target_csr))
    else:
        candidates = list(SupervisorCsrCandidates)
        weights = [1.0] * len(candidates)
        if corestate.interrupt_block:
            for csr in (
                SupervisorCsrCandidates.SCAUSE,
                SupervisorCsrCandidates.SIE,
            ):
                idx = candidates.index(csr)
                weights[idx] = 0.0

        target_csr = test_params.prng.choices(candidates, weights=weights, k=1)[0]
        if target_csr == SupervisorCsrCandidates.SCAUSE:
            if (
                "vexriscv" in test_params.design_name
            ):  # vexriscv complies with the privileged spec v1.10, which does
                # not require scause to hold the 5th bit. Similarly, kronos
                # implements privileged spec v1.11
                rd = corestate.intregpickstate.pick_int_outputreg()
                ret = CSRImmInstr(
                    "csrrwi", rd, test_params.prng.randrange(16), CSR.SCAUSE
                )
            else:
                rd = corestate.intregpickstate.pick_int_outputreg()
                ret = CSRImmInstr(
                    "csrrwi", rd, test_params.prng.randrange(32), CSR.SCAUSE
                )
        elif target_csr == SupervisorCsrCandidates.SSCRATCH:
            rs1 = corestate.intregpickstate.pick_int_inputreg()
            rd = corestate.intregpickstate.pick_int_outputreg()
            ret = CSRRegInstr("csrrw", rd, rs1, CSR.SSCRATCH)
        elif target_csr == SupervisorCsrCandidates.SIE:
            rd = corestate.intregpickstate.pick_int_inputreg()
            ret = CSRRegInstr("csrrs", rd, IntReg.zero, CSR.SIE)
        else:
            raise Exception("Unexpected target_csr: {}".format(target_csr))
    return ret
