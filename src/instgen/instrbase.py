# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


# This script defines.

from instgen.instrabstract import RVInstr, ImmInstr, CSRInstr
from riscv import IntReg, PrivLvl
from asm import twos_complement
from riscv import *
from random import Random
from typing import Optional

# These classes are here for generating multi-instruction fuzzing programs.


###
# Concrete classes: integers
###

# Instr with rs1, rs2 and rd
R12DInstrs = (
    "add",
    "sub",
    "sll",
    "slt",
    "sltu",
    "xor",
    "srl",
    "sra",
    "or",
    "and",
    "addw",
    "subw",
    "sllw",
    "srlw",
    "sraw",
    "mul",
    "mulh",
    "mulhsu",
    "mulhu",
    "div",
    "divu",
    "rem",
    "remu",
    "mulw",
    "divw",
    "divuw",
    "remw",
    "remuw",
)


class R12DInstr(RVInstr):
    authorized_instr_strs = R12DInstrs

    def __init__(
        self,
        instr_str: str,
        rd: IntReg,
        rs1: IntReg,
        rs2: IntReg,
        iscompressed: bool = False,
    ):
        self.rs1 = rs1
        self.rs2 = rs2
        self.rd = rd
        super().__init__(instr_str, iscompressed)

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # rv32i
        if self.instr_str == "add":
            return rv32i_add(self.rd.value, self.rs1.value, self.rs2.value)
        elif self.instr_str == "sub":
            return rv32i_sub(self.rd.value, self.rs1.value, self.rs2.value)
        elif self.instr_str == "sll":
            return rv32i_sll(self.rd.value, self.rs1.value, self.rs2.value)
        elif self.instr_str == "slt":
            return rv32i_slt(self.rd.value, self.rs1.value, self.rs2.value)
        elif self.instr_str == "sltu":
            return rv32i_sltu(self.rd.value, self.rs1.value, self.rs2.value)
        elif self.instr_str == "xor":
            return rv32i_xor(self.rd.value, self.rs1.value, self.rs2.value)
        elif self.instr_str == "srl":
            return rv32i_srl(self.rd.value, self.rs1.value, self.rs2.value)
        elif self.instr_str == "sra":
            return rv32i_sra(self.rd.value, self.rs1.value, self.rs2.value)
        elif self.instr_str == "or":
            return rv32i_or(self.rd.value, self.rs1.value, self.rs2.value)
        elif self.instr_str == "and":
            return rv32i_and(self.rd.value, self.rs1.value, self.rs2.value)
        # rv64i
        elif self.instr_str == "addw":
            return rv64i_addw(self.rd.value, self.rs1.value, self.rs2.value)
        elif self.instr_str == "subw":
            return rv64i_subw(self.rd.value, self.rs1.value, self.rs2.value)
        elif self.instr_str == "sllw":
            return rv64i_sllw(self.rd.value, self.rs1.value, self.rs2.value)
        elif self.instr_str == "srlw":
            return rv64i_srlw(self.rd.value, self.rs1.value, self.rs2.value)
        elif self.instr_str == "sraw":
            return rv64i_sraw(self.rd.value, self.rs1.value, self.rs2.value)
        # rv32m
        elif self.instr_str == "mul":
            return rv32m_mul(self.rd.value, self.rs1.value, self.rs2.value)
        elif self.instr_str == "mulh":
            return rv32m_mulh(self.rd.value, self.rs1.value, self.rs2.value)
        elif self.instr_str == "mulhsu":
            return rv32m_mulhsu(self.rd.value, self.rs1.value, self.rs2.value)
        elif self.instr_str == "mulhu":
            return rv32m_mulhu(self.rd.value, self.rs1.value, self.rs2.value)
        elif self.instr_str == "div":
            return rv32m_div(self.rd.value, self.rs1.value, self.rs2.value)
        elif self.instr_str == "divu":
            return rv32m_divu(self.rd.value, self.rs1.value, self.rs2.value)
        elif self.instr_str == "rem":
            return rv32m_rem(self.rd.value, self.rs1.value, self.rs2.value)
        elif self.instr_str == "remu":
            return rv32m_remu(self.rd.value, self.rs1.value, self.rs2.value)
        # rv64m
        elif self.instr_str == "mulw":
            return rv64m_mulw(self.rd.value, self.rs1.value, self.rs2.value)
        elif self.instr_str == "divw":
            return rv64m_divw(self.rd.value, self.rs1.value, self.rs2.value)
        elif self.instr_str == "divuw":
            return rv64m_divuw(self.rd.value, self.rs1.value, self.rs2.value)
        elif self.instr_str == "remw":
            return rv64m_remw(self.rd.value, self.rs1.value, self.rs2.value)
        elif self.instr_str == "remuw":
            return rv64m_remuw(self.rd.value, self.rs1.value, self.rs2.value)
        # Default case
        else:
            raise ValueError(f"Unexpected instruction string: `{self.instr_str}`.")


# Instr with imm and rd
ImmRdInstrs = ("lui", "auipc")


class ImmRdInstr(ImmInstr):
    authorized_instr_strs = ImmRdInstrs

    def __init__(
        self,
        instr_str: str,
        rd: IntReg,
        imm: int,
        is_rv64: bool,
        iscompressed: bool = False,
    ):
        self.imm = imm
        self.rd = rd
        super().__init__(instr_str, imm, is_rv64, iscompressed)

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # rv32i
        if self.instr_str == "lui":
            return rv32i_lui(self.rd.value, self.imm)
        elif self.instr_str == "auipc":
            return rv32i_auipc(self.rd.value, self.imm)
        # Default case
        else:
            raise ValueError(f"Unexpected instruction string: `{self.instr_str}`.")


# Instr with rs1, imm and rd
RegImmInstrs = (
    "addi",
    "slti",
    "sltiu",
    "xori",
    "ori",
    "andi",
    "slli",
    "srli",
    "srai",
    "addiw",
    "slliw",
    "srliw",
    "sraiw",
)


class RegImmInstr(ImmInstr):
    authorized_instr_strs = RegImmInstrs

    def __init__(
        self,
        instr_str: str,
        rd: IntReg,
        rs1: IntReg,
        imm: int,
        is_rv64: bool,
        iscompressed: bool = False,
    ):
        self.rs1 = rs1
        self.rd = rd
        super().__init__(instr_str, imm, is_rv64, iscompressed)
        if instr_str == "sraiw":
            assert self.imm >= 0

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # rv32i
        if self.instr_str == "addi":
            return rv32i_addi(self.rd.value, self.rs1.value, self.imm)
        elif self.instr_str == "slti":
            return rv32i_slti(self.rd.value, self.rs1.value, self.imm)
        elif self.instr_str == "sltiu":
            return rv32i_sltiu(self.rd.value, self.rs1.value, self.imm)
        elif self.instr_str == "xori":
            return rv32i_xori(self.rd.value, self.rs1.value, self.imm)
        elif self.instr_str == "ori":
            return rv32i_ori(self.rd.value, self.rs1.value, self.imm)
        elif self.instr_str == "andi":
            return rv32i_andi(self.rd.value, self.rs1.value, self.imm)
        elif self.instr_str == "slli":
            return rv32i_slli(self.rd.value, self.rs1.value, self.imm)
        elif self.instr_str == "srli":
            return rv32i_srli(self.rd.value, self.rs1.value, self.imm)
        elif self.instr_str == "srai":
            return rv32i_srai(self.rd.value, self.rs1.value, self.imm)
        # rv64i
        elif self.instr_str == "addiw":
            return rv64i_addiw(self.rd.value, self.rs1.value, self.imm)
        elif self.instr_str == "slliw":
            return rv64i_slliw(self.rd.value, self.rs1.value, self.imm)
        elif self.instr_str == "srliw":
            return rv64i_srliw(self.rd.value, self.rs1.value, self.imm)
        elif self.instr_str == "sraiw":
            return rv64i_sraiw(self.rd.value, self.rs1.value, self.imm)
        # Default case
        else:
            raise ValueError(f"Unexpected instruction string: `{self.instr_str}`.")


# Branch instructions: with rs1, rs2 and an immediate
BranchInstrs = ("beq", "bne", "blt", "bge", "bltu", "bgeu")


class BranchInstr(ImmInstr):
    authorized_instr_strs = BranchInstrs

    def __init__(
        self,
        instr_str: str,
        rs1: IntReg,
        rs2: IntReg,
        imm: int,
        plan_taken: bool,
        is_rv64: bool,
        fixed_opcode: bool = False,
        iscompressed: bool = False,
    ):
        self.rs1 = rs1
        self.rs2 = rs2
        self.plan_taken = plan_taken
        self.fixed_opcode = fixed_opcode
        super().__init__(instr_str, imm, is_rv64, iscompressed)

    # Choose an opcode that, given the values of rs1 and rs2, will comply with the required takenness
    def select_suitable_opcode(self, rs1_content: int, rs2_content: int, prng: Random):
        # Simulate branch behavior for each branch types
        int_plan_taken = int(self.plan_taken)
        beq = rs1_content != rs2_content
        bne = rs1_content == rs2_content
        blt = twos_complement(rs1_content, self.is_rv64) >= twos_complement(
            rs2_content, self.is_rv64
        )
        bge = twos_complement(rs1_content, self.is_rv64) < twos_complement(
            rs2_content, self.is_rv64
        )
        bltu = rs1_content >= rs2_content
        bgeu = rs1_content < rs2_content
        can_take_opcodes = [
            int_plan_taken ^ int(beq),
            int_plan_taken ^ int(bne),
            int_plan_taken ^ int(blt),
            int_plan_taken ^ int(bge),
            int_plan_taken ^ int(bltu),
            int_plan_taken ^ int(bgeu),
        ]
        self.instr_str = prng.choices(BranchInstrs, can_take_opcodes, k=1)[0]

    def gen_bytecode_int(self, is_spike_resolution: bool):
        if is_spike_resolution and not self.fixed_opcode:
            if self.plan_taken:
                # Just unconditionally jump to the next basic block
                return rv32i_jal(0, self.imm)
            else:
                # Nop
                return rv32i_addi(0, 0, 0)
        else:
            # rv32i
            if self.instr_str == "beq":
                return rv32i_beq(self.rs1.value, self.rs2.value, self.imm)
            elif self.instr_str == "bne":
                return rv32i_bne(self.rs1.value, self.rs2.value, self.imm)
            elif self.instr_str == "blt":
                return rv32i_blt(self.rs1.value, self.rs2.value, self.imm)
            elif self.instr_str == "bge":
                return rv32i_bge(self.rs1.value, self.rs2.value, self.imm)
            elif self.instr_str == "bltu":
                return rv32i_bltu(self.rs1.value, self.rs2.value, self.imm)
            elif self.instr_str == "bgeu":
                return rv32i_bgeu(self.rs1.value, self.rs2.value, self.imm)
            # Default case
            else:
                raise ValueError(f"Unexpected instruction string: `{self.instr_str}`.")


# The jal instruction
JALInstrs = ("jal",)


class JALInstr(ImmInstr):
    authorized_instr_strs = JALInstrs

    def __init__(
        self,
        instr_str: str,
        rd: IntReg,
        imm: int,
        is_absolute: bool = False,
        iscompressed: bool = False,
    ):
        # 32 or 64 bit does not matter for JAL
        self.rd = rd
        self.is_absolute = is_absolute
        super().__init__(instr_str, imm, False, iscompressed)

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # rv32i
        return rv32i_jal(self.rd.value, self.imm)


# The jalr instruction
JALRInstrs = ("jalr",)


class JALRInstr(ImmInstr):
    authorized_instr_strs = JALRInstrs

    def __init__(
        self,
        instr_str: str,
        rd: IntReg,
        rs1: IntReg,
        imm: int,
        producer_id: Optional[int],
        is_rv64: bool,
        iscompressed: bool = False,
    ):
        self.rd = rd
        self.rs1 = rs1
        self.producer_id = producer_id
        super().__init__(instr_str, imm, is_rv64, iscompressed)

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # rv32i
        return rv32i_jalr(self.rd.value, self.rs1.value, self.imm)


# Instr that create no information flow
FenceInstrs = ("fence", "fence.i")


class FenceInstr(RVInstr):
    authorized_instr_strs = FenceInstrs

    def __init__(
        self,
        instr_str: str,
        fence_ordering: FenceOrdering,
        iscompressed: bool = False,
    ):
        self.fence_ordering = fence_ordering
        super().__init__(instr_str, iscompressed)

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # rv32i
        if self.instr_str == "fence":
            return rv32i_fence(self.fence_ordering.value)
        # zifencei
        elif self.instr_str == "fence.i":
            return zifencei_fencei(self.fence_ordering.value)
        # Default case
        else:
            raise ValueError(f"Unexpected instruction string: `{self.instr_str}`.")


EcallEbreakInstrs = ("ecall", "ebreak")


class EcallEbreakInstr(RVInstr):
    authorized_instr_strs = EcallEbreakInstrs

    def __init__(self, instr_str: str, iscompressed: bool = False):
        super().__init__(instr_str, iscompressed)

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # rv32i
        if self.instr_str == "ecall":
            return rv32i_ecall()
        elif self.instr_str == "ebreak":
            return rv32i_ebreak()
        # Default case
        else:
            raise ValueError(f"Unexpected instruction string: `{self.instr_str}`.")


# Integer load instructions
IntLoadInstrs = ("lb", "lh", "lw", "lbu", "lhu", "lwu", "ld")


class IntLoadInstr(ImmInstr):
    authorized_instr_strs = IntLoadInstrs

    def __init__(
        self,
        instr_str: str,
        rd: IntReg,
        rs1: IntReg,
        imm: int,
        producer_id: Optional[int],
        is_rv64: bool,
        iscompressed: bool = False,
        dest_addr: Optional[int] = None,
        is_interrupt: bool = False,
    ):
        self.rd = rd
        self.rs1 = rs1
        self.producer_id = producer_id
        self.dest_addr = dest_addr
        self.is_interrupt = is_interrupt
        super().__init__(instr_str, imm, is_rv64, iscompressed)

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # rv32i
        if self.instr_str == "lb":
            return rv32i_lb(self.rd.value, self.rs1.value, self.imm)
        elif self.instr_str == "lh":
            return rv32i_lh(self.rd.value, self.rs1.value, self.imm)
        elif self.instr_str == "lw":
            return rv32i_lw(self.rd.value, self.rs1.value, self.imm)
        elif self.instr_str == "lbu":
            return rv32i_lbu(self.rd.value, self.rs1.value, self.imm)
        elif self.instr_str == "lhu":
            return rv32i_lhu(self.rd.value, self.rs1.value, self.imm)
        # rv64i
        elif self.instr_str == "lwu":
            return rv64i_lwu(self.rd.value, self.rs1.value, self.imm)
        elif self.instr_str == "ld":
            return rv64i_ld(self.rd.value, self.rs1.value, self.imm)
        # Default caextends Regs(Enumse
        else:
            raise ValueError(f"Unexpected instruction string: `{self.instr_str}`.")


# Integer store instructions
IntStoreInstrs = ("sb", "sh", "sw", "sd")


class IntStoreInstr(ImmInstr):
    authorized_instr_strs = IntStoreInstrs

    def __init__(
        self,
        instr_str: str,
        rs1: IntReg,
        rs2: IntReg,
        imm: int,
        producer_id: Optional[int],
        is_rv64: bool,
        iscompressed: bool = False,
        dest_addr: Optional[int] = None,
        is_interrupt: bool = False,
    ):
        self.rs1 = rs1
        self.rs2 = rs2
        self.producer_id = producer_id
        self.dest_addr = dest_addr
        self.is_interrupt = is_interrupt
        super().__init__(instr_str, imm, is_rv64, iscompressed)

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # rv32i
        if self.instr_str == "sb":
            return rv32i_sb(self.rs1.value, self.rs2.value, self.imm)
        elif self.instr_str == "sh":
            return rv32i_sh(self.rs1.value, self.rs2.value, self.imm)
        elif self.instr_str == "sw":
            return rv32i_sw(self.rs1.value, self.rs2.value, self.imm)
        # rv64i
        elif self.instr_str == "sd":
            return rv64i_sd(self.rs1.value, self.rs2.value, self.imm)
        # Default case
        else:
            raise ValueError(f"Unexpected instruction string: `{self.instr_str}`.")


###
# CSR instructions
###

# CSR operations without immediate
CSRRegInstrs = ("csrrw", "csrrs", "csrrc")


class CSRRegInstr(CSRInstr):
    authorized_instr_strs = CSRRegInstrs

    def __init__(
        self,
        instr_str: str,
        rd: IntReg,
        rs1: IntReg,
        csr_id: int,
        iscompressed: bool = False,
    ):
        self.rd = rd
        self.rs1 = rs1
        super().__init__(instr_str, csr_id, iscompressed)

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # rv32i
        if self.instr_str == "csrrw":
            return zicsr_csrrw(self.rd.value, self.rs1.value, self.csr_id)
        elif self.instr_str == "csrrs":
            return zicsr_csrrs(self.rd.value, self.rs1.value, self.csr_id)
        elif self.instr_str == "csrrc":
            return zicsr_csrrc(self.rd.value, self.rs1.value, self.csr_id)
        # Default case
        else:
            raise ValueError(f"Unexpected instruction string: `{self.instr_str}`.")

    def get_inputregs(self) -> list[IntReg | FloatReg | CSR]:
        """Returns a list of input registers"""
        if self.instr_str == "csrrw" and self.rd == IntReg.zero:
            return [self.rs1]
        elif self.csr_id not in [e.value for e in CSR]:
            return [self.rs1]
        else:
            return [CSR(self.csr_id), self.rs1]

    def get_outputregs(self) -> list[IntReg | FloatReg | CSR]:
        """Returns a list of output registers"""
        csr_alias: list[IntReg | FloatReg | CSR] = []
        if self.csr_id == CSR.FCSR:
            csr_alias += [CSR.FRM, CSR.FFLAGS]
        elif self.csr_id in (CSR.FRM, CSR.FFLAGS):
            csr_alias += [CSR.FCSR]

        if self.csr_id not in [e.value for e in CSR]:
            if self.instr_str in ("csrrs", "csrrc") and self.rs1 == IntReg.zero:
                return csr_alias
            else:
                return [self.rs1] + csr_alias
        else:
            if self.instr_str in ("csrrs", "csrrc") and self.rs1 == IntReg.zero:
                return [CSR(self.csr_id)] + csr_alias
            else:
                return [CSR(self.csr_id), self.rs1] + csr_alias


# CSR operations with immediate
CSRImmInstrs = ("csrrwi", "csrrsi", "csrrci")


class CSRImmInstr(CSRInstr):
    authorized_instr_strs = CSRImmInstrs

    def __init__(
        self,
        instr_str: str,
        rd: IntReg,
        uimm: int,
        csr_id: int,
        iscompressed: bool = False,
    ):
        if __debug__:
            assert uimm >= 0
            assert uimm < 1 << 5
        self.rd = rd
        self.uimm = uimm
        super().__init__(instr_str, csr_id, iscompressed)

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # rv32i
        if self.instr_str == "csrrwi":
            return zicsr_csrrwi(self.rd.value, self.uimm, self.csr_id)
        elif self.instr_str == "csrrsi":
            return zicsr_csrrsi(self.rd.value, self.uimm, self.csr_id)
        elif self.instr_str == "csrrci":
            return zicsr_csrrci(self.rd.value, self.uimm, self.csr_id)
        # Default case
        else:
            raise ValueError(f"Unexpected instruction string: `{self.instr_str}`.")

    def get_inputregs(self) -> list[IntReg | FloatReg | CSR]:
        """Returns a list of input registers"""
        if self.instr_str == "csrrwi" and self.rd == IntReg.zero:
            return []
        else:
            return [CSR(self.csr_id)]

    def get_outputregs(self) -> list[IntReg | FloatReg | CSR]:
        """Returns a list of output registers"""
        csr_alias: list[IntReg | FloatReg | CSR] = []
        if self.csr_id == CSR.FCSR:
            csr_alias += [CSR.FRM, CSR.FFLAGS]
        elif self.csr_id in (CSR.FRM, CSR.FFLAGS):
            csr_alias += [CSR.FCSR]

        if self.instr_str in ("csrrsi", "csrrci") and self.uimm == 0:
            return [CSR(self.csr_id)] + csr_alias
        else:
            return [CSR(self.csr_id), self.rd] + csr_alias


PrivDescentInstrs = ("mret", "sret")


class PrivDescentInstr(RVInstr):
    authorized_instr_strs = PrivDescentInstrs

    def __init__(
        self,
        is_mret: bool,
        prev_priv: PrivLvl,
        will_trap: bool = False,
        is_helper: bool = False,
    ):
        if is_mret:
            instr_str = "mret"
        else:
            instr_str = "sret"
        self.is_mret = is_mret
        self.prev_priv = prev_priv
        self.will_trap = will_trap
        self.is_helper = is_helper
        super().__init__(instr_str)

    def gen_bytecode_int(self, is_spike_resolution: bool):
        if self.is_mret:
            return rvprivileged_mret()
        else:
            return rvprivileged_sret()


WfiInstrs = "wfi"


class WfiInstr(RVInstr):
    authorized_instr_strs = WfiInstrs

    def __init__(self):
        super().__init__("wfi")

    def gen_bytecode_int(self, is_spike_resolution: bool):
        return rvprivileged_wfi()
