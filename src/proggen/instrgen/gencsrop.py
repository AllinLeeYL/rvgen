# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only
from instgen import (
    TvecWriterInstr,
    EPCWriterInstr,
    MxdelegWriterInstr,
    CSRRegInstr,
    CSRImmInstr,
    RegImmInstr,
    TrapInstrWrapper,
    ImmRdInstr,
)
from riscv import PrivLvl, IntReg, CSR
from states.regstate import IntRegState
from preisasim import SPIKE_MEDELEG_MASK, SPIKE_MIDELEG_MASK
from params import (
    MPP_BOTH_BITS_REG,
    SPP_BIT_VAL,
    MPIE_BIT_VAL,
)
from enum import Enum, auto
from typing import TYPE_CHECKING
from random import Random

if TYPE_CHECKING:
    from states.corestate import CoreState
    from params import TestParams


class MachineCsrOps(Enum):
    Medelg = auto()
    Midelg = auto()
    MstatusMie = auto()
    MieMsie = auto()
    XppFsm = auto()
    MstatusMpie = auto()


def is_ops_available(corestate: "CoreState", test_params: "TestParams"):
    mcsr_ops, _ = _gen_takable_ops(corestate, test_params)
    if sum(mcsr_ops.values()) == 0:
        return False
    else:
        return True


def gen_mcsr_instr(corestate: "CoreState", test_params: "TestParams"):
    """Generates a new Machine mode CSR write instruction

    The returned instruction might trap due to pending interrupts.
    """
    will_trap = False
    mcsr_ops, set_clear_weight = _gen_takable_ops(corestate, test_params)
    op = test_params.prng.choices(
        list(mcsr_ops.keys()), weights=list(mcsr_ops.values())
    )[0]
    match op:
        case MachineCsrOps.Medelg:
            ret = _gen_medeleg_instr(corestate, test_params)
        case MachineCsrOps.Midelg:
            ret = _gen_mideleg_instr(corestate, test_params)
        case MachineCsrOps.MstatusMie:
            ret, will_trap = _gen_mstatus_mie_instr(
                corestate,
                list(set_clear_weight[MachineCsrOps.MstatusMie]),
                test_params.prng,
                test_params.design_name,
            )
        case MachineCsrOps.MieMsie:
            ret, will_trap = _gen_mie_msie_instr(
                corestate,
                list(set_clear_weight[MachineCsrOps.MieMsie]),
                test_params.prng,
            )
        case MachineCsrOps.XppFsm:
            ret = _gen_ppfill_instrs(corestate, test_params)
        case MachineCsrOps.MstatusMpie:
            ret = _gen_mstatus_mpie_instr(
                corestate, test_params.is_rv64, test_params.prng
            )
        case _:
            raise ValueError("Unknow M-mode CSR op")

    if will_trap:
        corestate.hartstate.is_mtvec_populated = False
        corestate.hartstate.mstatus_mpp = corestate.hartstate.privlvl
        corestate.hartstate.is_mepc_populated = False  # FIXME
        corestate.hartstate.privlvl = PrivLvl.Machine
        corestate.hartstate.mstatus_mpie = corestate.hartstate.mstatus_mie
        corestate.hartstate.mstatus_mie = False

    return ret, will_trap


def _gen_takable_ops(corestate: "CoreState", test_params: "TestParams"):
    mcsr_ops = {op: 1.0 for op in MachineCsrOps}
    set_clear_weight = {
        MachineCsrOps.MstatusMie: [True, True],
        MachineCsrOps.MieMsie: [True, True],
    }
    has_consumed_reg = corestate.intregpickstate.exists_reg_in_state(
        IntRegState.CONSUMED
    )
    if test_params.num_harts == 1:
        mcsr_ops[MachineCsrOps.MstatusMie] = 0
        mcsr_ops[MachineCsrOps.MieMsie] = 0
        mcsr_ops[MachineCsrOps.MstatusMpie] = 0
    if not test_params.s_mode_support:
        mcsr_ops[MachineCsrOps.Medelg] = 0
        mcsr_ops[MachineCsrOps.Midelg] = 0
    if "picorv32" in test_params.design_name:
        mcsr_ops[MachineCsrOps.XppFsm] = 0
    # FIXME eventually get rid of the consumed reg and just generate it
    # using shifts and the relocator reg ?
    if not has_consumed_reg:
        mcsr_ops[MachineCsrOps.Medelg] = 0
        mcsr_ops[MachineCsrOps.Midelg] = 0

    # FIXME until I find a good ACLINT implementation, we disable delegation
    mcsr_ops[MachineCsrOps.Midelg] = 0

    # Setting the bit will cause a trap, and there is no trap vector
    # availbale. Setting the bit is disabled, and the proba of
    # choosing this option is reduced. FIXME, update when delegation support is
    # added
    if (
        corestate.hartstate.mip_msip
        and corestate.hartstate.mie_msie
        and not corestate.hartstate.is_mtvec_populated
    ):
        set_clear_weight[MachineCsrOps.MstatusMie] = [False, True]
        mcsr_ops[MachineCsrOps.MstatusMie] = 0.1
    if (
        corestate.hartstate.mip_msip
        and corestate.hartstate.mstatus_mie
        and not corestate.hartstate.is_mtvec_populated
    ):
        set_clear_weight[MachineCsrOps.MieMsie] = [False, True]
        mcsr_ops[MachineCsrOps.MieMsie] = 0.1

    return mcsr_ops, set_clear_weight


def _gen_mstatus_mpie_instr(corestate: "CoreState", is_rv64: bool, prng: Random):
    """Randomly sets or clear the MPIE bit in the MSTATUS CSR"""
    set_bit = prng.choices([True, False], weights=[0.5, 0.5], k=1)[0]
    rs1 = corestate.intregpickstate.pick_int_outputreg_nonzero()
    if set_bit:
        corestate.hartstate.mstatus_mpie = True
        ret = [
            RegImmInstr("addi", rs1, IntReg.zero, MPIE_BIT_VAL, is_rv64),
            CSRRegInstr("csrrs", IntReg.zero, rs1, CSR.MSTATUS),
        ]
    else:
        corestate.hartstate.mstatus_mpie = False
        ret = [
            RegImmInstr("addi", rs1, IntReg.zero, MPIE_BIT_VAL, is_rv64),
            CSRRegInstr("csrrc", IntReg.zero, rs1, CSR.MSTATUS),
        ]
    return ret


def _gen_mie_msie_instr(corestate: "CoreState", weights: list[bool], prng: Random):
    """Randomly sets or clears the MSIE bit in the MIE register

    If mie.MSIP is set and there is a pending interrupt, this instruction
    will trap
    """

    # TODO we could assign a heavier weight to the op that changes the state
    set_bit = prng.choices([True, False], weights=weights, k=1)[0]
    rd = corestate.intregpickstate.pick_int_outputreg()
    if set_bit:
        corestate.hartstate.mie_msie = True
        ret = CSRImmInstr("csrrsi", rd, 0x8, CSR.MIE)
    else:
        corestate.hartstate.mie_msie = False
        ret = CSRImmInstr("csrrci", rd, 0x8, CSR.MIE)
    will_trap = (
        set_bit and corestate.hartstate.mstatus_mie and corestate.hartstate.mip_msip
    )
    if will_trap:
        ret = TrapInstrWrapper(True, None, ret, corestate.hartstate.privlvl)
    return [ret], will_trap


def _gen_mstatus_mie_instr(
    corestate: "CoreState", weights: list[bool], prng: Random, design_name: str
):
    """randomly sets or clears the MIE bit in mstatus. We do not use the
    output, as th FS bit can change between the spike resolution and reduction.

    If mstatus.MIE is set and there is a pending interrupt, this instruction
    will trap
    """
    # TODO we could assign a heavier weight to the op that changes the state
    set_bit = prng.choices([True, False], weights=weights, k=1)[0]
    rd = IntReg.zero  # XiangShan
    if set_bit:
        corestate.hartstate.mstatus_mie = True
        ret = CSRImmInstr("csrrsi", rd, 0x8, CSR.MSTATUS)
    else:
        corestate.hartstate.mstatus_mie = False
        ret = CSRImmInstr("csrrci", rd, 0x8, CSR.MSTATUS)
    will_trap = (
        set_bit and corestate.hartstate.mie_msie and corestate.hartstate.mip_msip
    )
    if will_trap:
        ret = TrapInstrWrapper(True, None, ret, corestate.hartstate.privlvl)
    return [ret], will_trap


def _gen_mideleg_instr(corestate: "CoreState", test_params: "TestParams"):
    """Generates a sequence of instructions that updates the MIDELEG CSR value"""
    if __debug__:
        assert corestate.hartstate.privlvl == PrivLvl.Machine

    # Random values for both the supported and unsupported bits, but only for
    # the CPU.
    val_to_write_spike = 0
    val_to_write_cpu = 0
    supported_mideleg_bits = test_params.mideleg_mask & SPIKE_MIDELEG_MASK
    supported_mideleg_bits_arr = []
    while supported_mideleg_bits:
        supported_mideleg_bits_arr.append(supported_mideleg_bits & 1)
        supported_mideleg_bits >>= 1
    del supported_mideleg_bits
    for bit_id, bit_val in enumerate(supported_mideleg_bits_arr):
        random_bit = bit_val and test_params.prng.randint(0, 1)
        val_to_write_cpu |= random_bit << bit_id
        # If the bit is not supported by the CPU, set it to 0 for Spike, but set
        # it randomly for the CPU.
        if bit_val == 1:
            val_to_write_spike |= random_bit << bit_id

    # Get some consumed register
    rs1 = corestate.intregpickstate.pick_reg_in_state(IntRegState.CONSUMED)
    producer_id = corestate.intregpickstate.get_producer_id(rs1)
    corestate.intregpickstate.set_regstate(rs1, IntRegState.FREE)

    rd = corestate.intregpickstate.pick_int_outputreg()
    # Update the delegated state in our model
    corestate.hartstate.mideleg = val_to_write_spike

    return [
        MxdelegWriterInstr(
            CSR.MIDELEG, rd, rs1, producer_id, val_to_write_spike, val_to_write_cpu
        )
    ]


def _gen_medeleg_instr(corestate: "CoreState", test_params: "TestParams"):
    """Generates a sequence of instructions that updates the MEDELEG CSR value"""
    if __debug__:
        assert corestate.hartstate.privlvl == PrivLvl.Machine

    # Random values for both the supported and unsupported bits, but only for
    # the CPU.
    val_to_write_spike = 0
    val_to_write_cpu = 0
    supported_medeleg_bits = test_params.medeleg_mask & SPIKE_MEDELEG_MASK
    supported_medeleg_bits_arr = []
    while supported_medeleg_bits:
        supported_medeleg_bits_arr.append(supported_medeleg_bits & 1)
        supported_medeleg_bits >>= 1
    del supported_medeleg_bits
    for bit_id, bit_val in enumerate(supported_medeleg_bits_arr):
        random_bit = bit_val and test_params.prng.randint(0, 1)
        val_to_write_cpu |= random_bit << bit_id
        # If the bit is not supported by the CPU, set it to 0 for Spike, but set
        # it randomly for the CPU.
        if bit_val == 1:
            val_to_write_spike |= random_bit << bit_id

    # Get some consumed register
    rs1 = corestate.intregpickstate.pick_reg_in_state(IntRegState.CONSUMED)
    producer_id = corestate.intregpickstate.get_producer_id(rs1)
    corestate.intregpickstate.set_regstate(rs1, IntRegState.FREE)

    rd = corestate.intregpickstate.pick_int_outputreg()

    # Update the delegated state in our model
    corestate.hartstate.medeleg = val_to_write_spike

    return [
        MxdelegWriterInstr(
            CSR.MEDELEG, rd, rs1, producer_id, val_to_write_spike, val_to_write_cpu
        )
    ]


# @brief this function generates an instruction that will fill the xPP field of
# mstatus with the provided value.
# @return a CFInstrType that will fill the xPP field of mstatus with the
# provided value.
def _gen_ppfill_instrs(corestate: "CoreState", test_params: "TestParams"):
    if __debug__:
        assert corestate.hartstate.privlvl == PrivLvl.Machine, (
            "The function gen_ppfill_instrs should only be called in machine mode. Currently in "
            + str(corestate.hartstate.privlvl)
        )

    # Technically, spp can be written even if supervisor mode does not exist.
    # But leave this detail for the FUTURE.
    if test_params.s_mode_support:
        is_mpp = test_params.prng.random() < 0.5
    else:
        is_mpp = True
    is_rv64 = test_params.is_rv64
    # Ignore the return value of mstatus for now
    rd = IntReg.zero

    # Choose the target. It should be a valid target.
    if is_mpp:
        if not test_params.s_mode_support and not test_params.u_mode_support:
            target_privlvl = PrivLvl.Machine
        elif test_params.s_mode_support and not test_params.u_mode_support:
            target_privlvl = test_params.prng.choice(
                [PrivLvl.Supervisor, PrivLvl.Machine]
            )
        elif not test_params.s_mode_support and test_params.u_mode_support:
            target_privlvl = test_params.prng.choice([PrivLvl.User, PrivLvl.Machine])
        else:
            target_privlvl = test_params.prng.choice(
                [
                    PrivLvl.User,
                    PrivLvl.Supervisor,
                    PrivLvl.Machine,
                ]
            )
    else:
        if test_params.u_mode_support:
            target_privlvl = test_params.prng.choice([PrivLvl.User, PrivLvl.Supervisor])
        else:
            target_privlvl = PrivLvl.Supervisor

    if is_mpp:
        if target_privlvl == PrivLvl.User:
            ret = [CSRRegInstr("csrrc", rd, MPP_BOTH_BITS_REG[0], CSR.MSTATUS)]
        elif target_privlvl == PrivLvl.Supervisor:
            mpp_bit_reg = corestate.intregpickstate.pick_int_outputreg_nonzero()
            ret = [
                ImmRdInstr("lui", mpp_bit_reg, 0b1, is_rv64),  # mpp top bit
                CSRRegInstr("csrrs", rd, MPP_BOTH_BITS_REG[0], CSR.MSTATUS),
                CSRRegInstr("csrrc", rd, mpp_bit_reg, CSR.MSTATUS),
            ]
        elif target_privlvl == PrivLvl.Machine:
            ret = [CSRRegInstr("csrrs", rd, MPP_BOTH_BITS_REG[0], CSR.MSTATUS)]
        else:
            raise NotImplementedError("Hypervisor mode not implemented")
    else:
        if target_privlvl == PrivLvl.User:
            mpp_bit_reg = corestate.intregpickstate.pick_int_outputreg_nonzero()
            ret = [
                RegImmInstr("addi", mpp_bit_reg, IntReg.zero, SPP_BIT_VAL, is_rv64),
                CSRRegInstr("csrrc", rd, mpp_bit_reg, CSR.MSTATUS),
            ]
        elif target_privlvl == PrivLvl.Supervisor:
            mpp_bit_reg = corestate.intregpickstate.pick_int_outputreg_nonzero()
            ret = [
                RegImmInstr("addi", mpp_bit_reg, IntReg.zero, SPP_BIT_VAL, is_rv64),
                CSRRegInstr("csrrs", rd, mpp_bit_reg, CSR.MSTATUS),
            ]
        else:
            raise Exception("Invalid target privlvl when setting spp")

    # Update the mpp/spp in our bookkeeping
    if is_mpp:
        corestate.hartstate.mstatus_mpp = target_privlvl
    else:
        corestate.hartstate.mstatus_spp = target_privlvl

    return ret


# @brief this function generates an instruction that will fill the tvec with
# the provided value.
# @return a CFInstrType that will fill the tvec with the provided value.
def gen_tvecfill_instr(corestate: "CoreState", test_params: "TestParams"):
    can_populate_mtvec = (
        corestate.hartstate.privlvl == PrivLvl.Machine
        and not corestate.hartstate.is_mtvec_populated
    )
    can_populate_stvec = (
        corestate.hartstate.privlvl == PrivLvl.Supervisor
        and not corestate.hartstate.is_stvec_populated
    ) or (
        corestate.hartstate.privlvl == PrivLvl.Machine
        and test_params.s_mode_support
        and not corestate.hartstate.is_stvec_populated
    )

    if __debug__:
        assert can_populate_mtvec or can_populate_stvec

    is_mtvec = True
    # Choose between mtvec and stvec
    if can_populate_mtvec and can_populate_stvec:
        is_mtvec = test_params.prng.random() < 0.5
    elif can_populate_mtvec and not can_populate_stvec:
        is_mtvec = True
    else:
        is_mtvec = False

    # Get some consumed register
    rs1 = corestate.intregpickstate.pick_reg_in_state(IntRegState.CONSUMED)
    producer_id = corestate.intregpickstate.get_producer_id(rs1)
    corestate.intregpickstate.set_regstate(rs1, IntRegState.FREE)

    # If this is the first write to the reg, then the reset val should be
    # ignored
    if is_mtvec:
        if corestate.hartstate.is_mtvec_non_deterministic:
            corestate.hartstate.is_mtvec_non_deterministic = False
            rd = IntReg.zero
        else:
            rd = corestate.intregpickstate.pick_int_outputreg()
        corestate.hartstate.is_mtvec_populated = True
    else:
        if corestate.hartstate.is_stvec_still_reset_val:
            corestate.hartstate.is_stvec_still_reset_val = False
            rd = IntReg.zero
        else:
            rd = corestate.intregpickstate.pick_int_outputreg()
        corestate.hartstate.is_stvec_populated = True

    return TvecWriterInstr(is_mtvec, rd, rs1, producer_id)


# @brief this function generates an instruction that will fill the epc with the provided value.
# @return a CFInstrType that will fill the epc with the provided value.
def gen_epcfill_instr(corestate: "CoreState", test_params: "TestParams"):
    if __debug__:
        assert corestate.hartstate.privlvl != PrivLvl.User

    can_populate_mepc = (
        corestate.hartstate.privlvl == PrivLvl.Machine
        and not corestate.hartstate.is_mepc_populated
    )
    can_populate_sepc = (
        corestate.hartstate.privlvl == PrivLvl.Supervisor
        and not corestate.hartstate.is_sepc_populated
    ) or (
        corestate.hartstate.privlvl == PrivLvl.Machine
        and test_params.s_mode_support
        and not corestate.hartstate.is_sepc_populated
    )

    if __debug__:
        assert can_populate_mepc or can_populate_sepc

    is_mepc = True  # If True, then mepc, else sepc

    # Choose between mepc and sepc
    if can_populate_mepc:
        if can_populate_sepc:
            is_mepc = test_params.prng.random() < 0.5
        else:
            is_mepc = True
    else:
        is_mepc = False

    # Get some consumed register
    rs1 = corestate.intregpickstate.pick_reg_in_state(IntRegState.CONSUMED)
    producer_id = corestate.intregpickstate.get_producer_id(rs1)
    corestate.intregpickstate.set_regstate(rs1, IntRegState.FREE)

    # If this is the first write to the reg, then the reset val should be ignored
    if is_mepc:
        if corestate.hartstate.is_mepc_non_deterministic:
            corestate.hartstate.is_mepc_non_deterministic = False
            rd = IntReg.zero
        else:
            rd = corestate.intregpickstate.pick_int_outputreg()
        corestate.hartstate.is_mepc_populated = True
    else:
        if corestate.hartstate.is_sepc_still_reset_val:
            corestate.hartstate.is_sepc_still_reset_val = False
            rd = IntReg.zero
        else:
            rd = corestate.intregpickstate.pick_int_outputreg()
        corestate.hartstate.is_sepc_populated = True

    return EPCWriterInstr(is_mepc, rd, rs1, producer_id)


def clear_mstatus_mdt(corestate: "CoreState", is_rv64: bool):
    assert corestate.hartstate.privlvl == PrivLvl.Machine
    corestate.hartstate.mstatus_mdt = False
    mdt_clear_reg = corestate.intregpickstate.pick_int_outputreg_nonzero()
    # MDT bit is at index 42 in mstatus (starting at idx 0)
    ret = [
        RegImmInstr("addi", mdt_clear_reg, IntReg.zero, 1, is_rv64),
        RegImmInstr("slli", mdt_clear_reg, mdt_clear_reg, 42, is_rv64),
        CSRRegInstr("csrrc", IntReg.zero, mdt_clear_reg, CSR.MSTATUS),
    ]
    return ret
