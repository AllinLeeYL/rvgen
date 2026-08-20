# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


from instgen.helper import INSTRUCTION_IDS
from instgen.instrbase import IntLoadInstr, IntStoreInstr, CSRRegInstr
from instgen.instrfloat import FloatLoadInstr, FloatStoreInstr
from instgen.instrabstract import TrapInstr, RVInstr, WrapperInstr
from params import (
    RELOCATOR_REG,
    RDEP_MASK_REG,
)
from asm import (
    li_into_reg,
    to_unsigned,
)
from riscv import *
from typing import Optional, TYPE_CHECKING

if TYPE_CHECKING:
    from states import CoreState
    from params import TestParams

###
# Placeholder instructions
###

# For offset producers and consumers.
# The offset producers and consumers do not yet know the final immediate values, and are used differently in the spike resolution from ..the actual simulation.
# 1. For the spike resolution
#   a. The offset producer creates the target address (or branch decision register).
#   b. The offset consumer forwards this offset which is also the target address in this case.
# 2. For the actual simulation
#   a. The offset producer computes an offset dependent on the resolution.
#   b. The offset consumer computes the generated, target address, by making the difference between the dependent register and the offset. This instruction does not require the spike resolution to be known, but still is different between the two scenari.


ProducerInstr0s = "lui"


class PlaceholderProducerInstr0(RVInstr):
    authorized_instr_strs = ProducerInstr0s

    # When it is instantiated, the producer instructions do not know the offset
    # yet, just the target address.
    def __init__(self, rd: IntReg, producer_id: Optional[int], is_rv64: bool):
        self.rd = rd
        self.producer_id = producer_id
        self.relocation_offset = 0
        self.spike_resolution_offset = None
        self.rtl_offset = None
        self.is_rv64 = is_rv64
        super().__init__("lui")

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # If this is the spike resolution, then load the target address using lui
        if is_spike_resolution:
            assert self.spike_resolution_offset != None
            if __debug__:
                assert self.spike_resolution_offset < (1 << 32)
            offset = self.spike_resolution_offset
        else:
            assert self.rtl_offset != None
            offset = self.rtl_offset
        return rv32i_lui(
            self.rd.value, li_into_reg(to_unsigned(offset, self.is_rv64), False)[0]
        )


ProducerInstr1s = "addi"


class PlaceholderProducerInstr1(RVInstr):
    authorized_instr_strs = ProducerInstr1s

    # When it is instantiated, the producer instructions do not know the offset
    # yet, just the target address.
    def __init__(self, rd: IntReg, producer_id: Optional[int], is_rv64: bool):
        self.rd = rd
        self.rs1 = rd
        self.producer_id = producer_id
        self.relocation_offset = 0
        self.spike_resolution_offset = None  # Is also the target address
        self.rtl_offset = None
        self.is_rv64 = is_rv64
        super().__init__("addi")

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # If this is the spike resolution, then load the target address using addi
        if is_spike_resolution:
            assert self.spike_resolution_offset != None
            if __debug__:
                assert self.spike_resolution_offset < (1 << 32)
            offset = self.spike_resolution_offset
        else:
            assert self.rtl_offset != None
            offset = self.rtl_offset
        return rv32i_addi(
            self.rd.value,
            self.rs1.value,
            li_into_reg(to_unsigned(offset, self.is_rv64), False)[1],
        )


PreConsumerInstrs = "and"


class PlaceholderPreConsumerInstr(RVInstr):
    """Used to restrict a register to 32bit, so we can produce it"""

    authorized_instr_strs = PreConsumerInstrs

    def __init__(self, reg: IntReg):
        self.rd = reg
        self.rs1 = reg
        self.rs2 = RDEP_MASK_REG[0]
        super().__init__("and")

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # Reduce the size of the reg id to 32 bits
        return rv32i_and(self.rd.value, self.rs1.value, self.rs2.value)


ConsumerInstrs = "xor"


class PlaceholderConsumerInstr(RVInstr):
    """
    PlaceholderConsumerInstr represents an instruction that consumes a register
    value, potentially creating a dependency on another register. It is used to
    generate bytecode for specific instruction patterns, with support for spike resolution scenarios.

    Attributes:
        rd (IntReg): The destination register (target address).
        rdep (IntReg): The register that creates the dependency. stored in rs2
        rprod (IntReg): The register produced by a previous instruction. stored in rs1
        producer_id (Optional[int]): Identifier for the producer, used for spike feedback.
    """

    authorized_instr_strs = ConsumerInstrs

    # @param rd: the generated register, i.e., the target address for example
    # @param rdep: the register that creates the dependency
    # @param producer_id: is required to feed spike's feedback
    def __init__(
        self, rd: IntReg, rdep: IntReg, rprod: IntReg, producer_id: Optional[int]
    ):
        self.rd = rd
        self.rs1 = rprod
        self.rs2 = rdep
        self.producer_id = producer_id
        super().__init__("xor")

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # If this is the spike resolution, then just transmit the produced
        # register
        if is_spike_resolution:
            # self.rprod - 0
            return rv32i_xor(self.rd.value, self.rs1.value, RELOCATOR_REG[0].value)
        else:
            # self.rdep - self.rprod
            return rv32i_xor(self.rd.value, self.rs1.value, self.rs2.value)


def is_placeholder(obj):
    return (
        isinstance(obj, PlaceholderProducerInstr0)
        or isinstance(obj, PlaceholderProducerInstr1)
        or isinstance(obj, PlaceholderPreConsumerInstr)
        or isinstance(obj, PlaceholderConsumerInstr)
    )


###
# Interrupts
###

InteruptInstrs = ("sb", "sw")


class InteruptInstr(WrapperInstr):
    """Generates a store instruction that sends an inter processor interrupt,
    discriminated, as it has a special purpose
    """

    authorized_instr_strs = InteruptInstrs

    def __init__(
        self,
        instr_str: str,
        rs1: IntReg,
        rs2: IntReg,
        imm: int,
        producer_id: int | None,
        target_core: int,
        is_rv64: bool,
        iscompressed: bool = False,
    ):
        self.target_core = target_core
        self.producer_id = producer_id
        instr = IntStoreInstr(
            instr_str, rs1, rs2, imm, producer_id, is_rv64, iscompressed
        )
        super().__init__(instr)


###
# Raw data
###

RawDataWords = ".insn"


class RawDataWord(RVInstr):
    authorized_instr_strs = RawDataWords

    def __init__(self, wordval: int, signed: bool = False):
        if __debug__:
            if signed:
                assert wordval >= -(1 << 31)
                assert wordval < (1 << 32), (
                    f"signed wordval: {wordval}, 1 << 32: {1 << 32}"
                )
            else:
                assert wordval >= 0
                assert wordval < (1 << 32), (
                    f"unsigned wordval: {hex(wordval)}, 1 << 32: {hex(1 << 32)}"
                )
        super().__init__(".insn")
        self.wordval = wordval
        if signed:
            if wordval < 0:
                self.wordval = wordval + (1 << 32)

    def gen_bytecode_int(self, is_spike_resolution: bool):
        return self.wordval


###
# CSR writers
###

TvecWriterInstrs = "csrrw"


class TvecWriterInstr(WrapperInstr):
    authorized_instr_strs = TvecWriterInstrs

    def __init__(
        self, is_mtvec: bool, rd: IntReg, rs1: IntReg, producer_id: Optional[int]
    ):
        self.producer_id = producer_id
        self.is_mtvec = is_mtvec
        csr_id = CSR.MTVEC if is_mtvec else CSR.STVEC
        csr_instr = CSRRegInstr("csrrw", rd, rs1, csr_id)
        super().__init__(csr_instr)


EPCWriterInstrs = "csrrw"


class EPCWriterInstr(WrapperInstr):
    authorized_instr_strs = EPCWriterInstrs

    def __init__(
        self, is_mepc: bool, rd: IntReg, rs1: IntReg, producer_id: Optional[int]
    ):
        self.producer_id = producer_id
        self.is_mepc = is_mepc
        csr_id = CSR.MEPC if is_mepc else CSR.SEPC
        csr_instr = CSRRegInstr("csrrw", rd, rs1, csr_id)
        super().__init__(csr_instr)


GenericCSRWriterInstrs = "csrrw"


class MxdelegWriterInstr(WrapperInstr):
    authorized_instr_strs = GenericCSRWriterInstrs

    def __init__(
        self,
        csr_id: int,
        rd: IntReg,
        rs1: IntReg,
        producer_id: Optional[int],
        val_to_write_spike: int,
        val_to_write_cpu: int,
    ):
        if __debug__:
            assert csr_id in (CSR.MIDELEG, CSR.MEDELEG)
        self.producer_id = producer_id
        self.val_to_write_spike = val_to_write_spike
        self.val_to_write_cpu = val_to_write_cpu
        csr_instr = CSRRegInstr("csrrw", rd, rs1, csr_id)
        super().__init__(csr_instr)


###
# For Trap (Interrupts and exceptions)
###


class TrapInstrWrapper(TrapInstr):
    """A wrapper for instructions that will trap: exception and interrupts"""

    authorized_instr_strs = list(INSTRUCTION_IDS.keys()) + [".insn"]

    def __init__(
        self,
        is_mtvec: bool,
        producer_id: Optional[int],
        instr: RVInstr,
        prev_priv: PrivLvl,
        special_interrupt_bb: bool = False,
    ):
        if __debug__:
            assert producer_id is None, "TrapInstrWrapper does not support producer_ids"
        self.instr = instr
        self.special_interrupt_bb = special_interrupt_bb
        super().__init__(instr, is_mtvec, prev_priv, producer_id)


class SimpleIllegalInstr(TrapInstr):
    authorized_instr_strs = RawDataWords

    def __init__(self, is_mtvec: bool, prev_priv: PrivLvl):
        instr = RawDataWord(0x00000000)
        super().__init__(instr, is_mtvec, prev_priv, None)


# This is a wrapper class for a misaligned load or store.
# As opposed to usual load and store operations used above, this class chooses
# a consumed register by itself.
MisalignedMemInstrs = (
    "lh",
    "lw",
    "lhu",
    "lwu",
    "ld",
    "sh",
    "sw",
    "sd",
    "flw",
    "fsw",
    "fld",
    "fsd",
)


class MisalignedMemInstr(TrapInstr):
    MISALIGNED_LH = 0
    MISALIGNED_LW = 1
    MISALIGNED_LHU = 2
    MISALIGNED_LWU = 3
    MISALIGNED_LD = 4
    MISALIGNED_SH = 5
    MISALIGNED_SW = 6
    MISALIGNED_SD = 7
    MISALIGNED_FLW = 8  # Requires F extension
    MISALIGNED_FSW = 9
    MISALIGNED_FLD = 10  # Requires D extension
    MISALIGNED_FSD = 11

    authorized_instr_strs = MisalignedMemInstrs

    def __init__(
        self,
        is_mtvec: bool,
        corestate: "CoreState",
        test_params: "TestParams",
        is_load: bool,
        prev_priv: PrivLvl,
        rs1: IntReg,
        producer_id: int,
        iscompressed: bool = False,
    ):
        # First, choose a consumed register.
        self.rs1 = rs1
        self.producer_id = producer_id

        # Second, choose a random memory instruction type that can be misaligned.
        is_fpu_activated = corestate.hartstate.mstatus_fs != FpuState.Off
        meminstr_type_weights = [
            is_load,  # MISALIGNED_LH
            is_load,  # MISALIGNED_LW
            is_load and test_params.is_rv64,  # MISALIGNED_LHU
            is_load and test_params.is_rv64,  # MISALIGNED_LWU
            is_load and test_params.is_rv64,  # MISALIGNED_LD
            not is_load,  # MISALIGNED_SH
            not is_load,  # MISALIGNED_SW
            (not is_load) and test_params.is_rv64,  # MISALIGNED_SD
            is_load and is_fpu_activated,  # MISALIGNED_FLW
            (not is_load) and is_fpu_activated,  # MISALIGNED_FSW
            is_load and is_fpu_activated,  # MISALIGNED_FLD
            (not is_load) and is_fpu_activated,  # MISALIGNED_FSD
        ]
        meminstr_type = test_params.prng.choices(
            range(len(meminstr_type_weights)), meminstr_type_weights
        )[0]

        # Third, the destination register for loads, and the source register for
        # stores does not matter because will not be architecturally accessed.
        random_intreg = corestate.intregpickstate.pick_random_integer_reg()
        random_floatreg = corestate.floatregpickstate.pick_random_float_reg()

        # Finally, pick a readable or writable address, since page or access
        # faults would have priority
        memrange = 0, test_params.memsize
        # Choose a misaligned address in the range
        if meminstr_type in (
            MisalignedMemInstr.MISALIGNED_LH,
            MisalignedMemInstr.MISALIGNED_LHU,
            MisalignedMemInstr.MISALIGNED_SH,
        ):
            curr_access_size = 2
        elif meminstr_type in (
            MisalignedMemInstr.MISALIGNED_LW,
            MisalignedMemInstr.MISALIGNED_LWU,
            MisalignedMemInstr.MISALIGNED_SW,
            MisalignedMemInstr.MISALIGNED_FLW,
            MisalignedMemInstr.MISALIGNED_FSW,
        ):
            curr_access_size = 4
        else:
            curr_access_size = 8
        memrange_base, memrange_size = memrange[0], memrange[1] - memrange[0]
        if __debug__:
            assert memrange_base >= 0, "memrange_base: %d" % memrange_base
            assert memrange_base + memrange_size <= test_params.memsize, (
                "memrange_base: %d, memrange_size: %d, memsize: %d"
                % (
                    memrange_base,
                    memrange_size,
                    test_params.memsize,
                )
            )
            assert memrange_size > curr_access_size, (
                "memrange_size: %d, curr_access_size: %d"
                % (
                    memrange_size,
                    curr_access_size,
                )
            )

        random_block = (
            test_params.prng.randrange(memrange_base, memrange_base + memrange_size)
            // curr_access_size
        ) * curr_access_size
        random_offset = test_params.prng.randrange(1, curr_access_size)
        self.misaligned_addr = random_block + random_offset

        if __debug__:
            assert self.misaligned_addr >= memrange_base, (
                "misaligned_addr: %d, memrange_base: %d"
                % (
                    self.misaligned_addr,
                    memrange_base,
                )
            )
            assert self.misaligned_addr < memrange_base + memrange_size, (
                "misaligned_addr: %d, memrange_base: %d, memrange_size: %d"
                % (
                    self.misaligned_addr,
                    memrange_base,
                    memrange_size,
                )
            )
            assert self.misaligned_addr % curr_access_size != 0, (
                "misaligned_addr: %d, curr_access_size: %d"
                % (
                    self.misaligned_addr,
                    curr_access_size,
                )
            )

        # Instantiate the wrapped instruction TODO store tuple (type, instr_str)
        imm = 0
        if meminstr_type == MisalignedMemInstr.MISALIGNED_LH:
            meminstr = IntLoadInstr(
                "lh",
                random_intreg,
                self.rs1,
                imm,
                self.producer_id,
                test_params.is_rv64,
                iscompressed,
            )
        elif meminstr_type == MisalignedMemInstr.MISALIGNED_LW:
            meminstr = IntLoadInstr(
                "lw",
                random_intreg,
                self.rs1,
                imm,
                self.producer_id,
                test_params.is_rv64,
                iscompressed,
            )
        elif meminstr_type == MisalignedMemInstr.MISALIGNED_LHU:
            meminstr = IntLoadInstr(
                "lhu",
                random_intreg,
                self.rs1,
                imm,
                self.producer_id,
                test_params.is_rv64,
                iscompressed,
            )
        elif meminstr_type == MisalignedMemInstr.MISALIGNED_LWU:
            meminstr = IntLoadInstr(
                "lwu",
                random_intreg,
                self.rs1,
                imm,
                self.producer_id,
                test_params.is_rv64,
                iscompressed,
            )
        elif meminstr_type == MisalignedMemInstr.MISALIGNED_LD:
            if __debug__:
                assert test_params.design_has_fpu
            meminstr = IntLoadInstr(
                "ld",
                random_intreg,
                self.rs1,
                imm,
                self.producer_id,
                test_params.is_rv64,
                iscompressed,
            )
        elif meminstr_type == MisalignedMemInstr.MISALIGNED_SH:
            meminstr = IntStoreInstr(
                "sh",
                self.rs1,
                random_intreg,
                imm,
                self.producer_id,
                test_params.is_rv64,
                iscompressed,
            )
        elif meminstr_type == MisalignedMemInstr.MISALIGNED_SW:
            meminstr = IntStoreInstr(
                "sw",
                self.rs1,
                random_intreg,
                imm,
                self.producer_id,
                test_params.is_rv64,
                iscompressed,
            )
        elif meminstr_type == MisalignedMemInstr.MISALIGNED_SD:
            if __debug__:
                assert test_params.design_has_fpu
            meminstr = IntStoreInstr(
                "sd",
                self.rs1,
                random_intreg,
                imm,
                self.producer_id,
                test_params.is_rv64,
                iscompressed,
            )
        elif meminstr_type == MisalignedMemInstr.MISALIGNED_FLW:
            if __debug__:
                assert test_params.design_has_fpu
            meminstr = FloatLoadInstr(
                "flw",
                random_floatreg,
                self.rs1,
                imm,
                self.producer_id,
                test_params.is_rv64,
                iscompressed,
            )
        elif meminstr_type == MisalignedMemInstr.MISALIGNED_FSD:
            if __debug__:
                assert test_params.design_has_fpud
            meminstr = FloatStoreInstr(
                "fsd",
                self.rs1,
                random_floatreg,
                imm,
                self.producer_id,
                test_params.is_rv64,
                iscompressed,
            )
        elif meminstr_type == MisalignedMemInstr.MISALIGNED_FSW:
            if __debug__:
                assert test_params.design_has_fpu
            meminstr = FloatStoreInstr(
                "fsw",
                self.rs1,
                random_floatreg,
                imm,
                self.producer_id,
                test_params.is_rv64,
                iscompressed,
            )
        elif meminstr_type == MisalignedMemInstr.MISALIGNED_FLD:
            if __debug__:
                assert test_params.design_has_fpud
            meminstr = FloatLoadInstr(
                "fld",
                random_floatreg,
                self.rs1,
                imm,
                self.producer_id,
                test_params.is_rv64,
                iscompressed,
            )
        else:
            raise NotImplementedError("Unsupported meminstrtype: " + str(meminstr_type))

        super().__init__(meminstr, is_mtvec, prev_priv, self.producer_id)
