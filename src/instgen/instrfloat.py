# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


from riscv import CSR, FloatReg, IntReg
from instgen.instrabstract import RVInstr, ImmInstr
from riscv import *
from typing import Optional

###
# Floating-point
###

# Float load instructions
FloatLoadInstrs = ("flw", "fld")


class FloatLoadInstr(ImmInstr):
    authorized_instr_strs = FloatLoadInstrs

    def __init__(
        self,
        instr_str: str,
        frd: FloatReg,
        rs1: IntReg,
        imm: int,
        producer_id: Optional[int],
        is_rv64: bool,
        iscompressed: bool = False,
        dest_addr: Optional[int] = None,
    ):
        self.frd = frd
        self.rs1 = rs1
        self.producer_id = producer_id
        self.dest_addr = dest_addr
        super().__init__(instr_str, imm, is_rv64, iscompressed)

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # rv32f
        if self.instr_str == "flw":
            return rv32f_flw(self.frd.value, self.rs1.value, self.imm)
        # rv32d
        elif self.instr_str == "fld":
            return rv32d_fld(self.frd.value, self.rs1.value, self.imm)
        # Default case
        else:
            raise ValueError(f"Unexpected instruction string: `{self.instr_str}`.")


# Float store instructions
FloatStoreInstrs = ("fsw", "fsd")


class FloatStoreInstr(ImmInstr):
    authorized_instr_strs = FloatStoreInstrs

    def __init__(
        self,
        instr_str: str,
        rs1: IntReg,
        frs2: FloatReg,
        imm: int,
        producer_id: Optional[int],
        is_rv64: bool,
        iscompressed: bool = False,
        dest_addr: Optional[int] = None,
    ):
        self.rs1 = rs1
        self.frs2 = frs2
        self.producer_id = producer_id
        self.dest_addr = dest_addr
        super().__init__(instr_str, imm, is_rv64, iscompressed)

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # rv32f
        if self.instr_str == "fsw":
            return rv32f_fsw(self.rs1.value, self.frs2.value, self.imm)
        # rv32d
        elif self.instr_str == "fsd":
            return rv32d_fsd(self.rs1.value, self.frs2.value, self.imm)
        # Default case
        else:
            raise ValueError(f"Unexpected instruction string: `{self.instr_str}`.")


# Float to int instructions
FloatToIntInstrs = (
    "fcvt.w.s",
    "fcvt.wu.s",
    "fcvt.l.s",
    "fcvt.lu.s",
    "fcvt.w.d",
    "fcvt.wu.d",
    "fcvt.l.d",
    "fcvt.lu.d",
)


class FloatToIntInstr(RVInstr):
    authorized_instr_strs = FloatToIntInstrs

    def __init__(
        self,
        instr_str: str,
        rd: IntReg,
        frs1: FloatReg,
        rm: int,
        iscompressed: bool = False,
    ):
        self.rm = rm
        self.frs1 = frs1
        self.rd = rd
        super().__init__(instr_str, iscompressed)

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # rv32f
        if self.instr_str == "fcvt.w.s":
            return rv32f_fcvtws(self.rd.value, self.frs1.value, self.rm)
        elif self.instr_str == "fcvt.wu.s":
            return rv32f_fcvtwus(self.rd.value, self.frs1.value, self.rm)
        # rv64f
        elif self.instr_str == "fcvt.l.s":
            return rv64f_fcvtls(self.rd.value, self.frs1.value, self.rm)
        elif self.instr_str == "fcvt.lu.s":
            return rv64f_fcvtlus(self.rd.value, self.frs1.value, self.rm)
        # rv32d
        elif self.instr_str == "fcvt.w.d":
            return rv32d_fcvtwd(self.rd.value, self.frs1.value, self.rm)
        # rv32d
        elif self.instr_str == "fcvt.wu.d":
            return rv32d_fcvtwud(self.rd.value, self.frs1.value, self.rm)
        # rv64d
        elif self.instr_str == "fcvt.l.d":
            return rv64d_fcvtld(self.rd.value, self.frs1.value, self.rm)
        elif self.instr_str == "fcvt.lu.d":
            return rv64d_fcvtlud(self.rd.value, self.frs1.value, self.rm)
        # Default case
        else:
            raise ValueError(f"Unexpected instruction string: `{self.instr_str}`.")

    def get_inputregs(self) -> list[IntReg | FloatReg | CSR]:
        if self.rm == 0b111:
            return [self.frs1, CSR.FRM]
        else:
            return [self.frs1]


# Int to float instructions
IntToFloatInstrs = (
    "fcvt.s.w",
    "fcvt.s.wu",
    "fcvt.s.l",
    "fcvt.s.lu",
    "fcvt.d.w",
    "fcvt.d.wu",
    "fcvt.d.l",
    "fcvt.d.lu",
)


class IntToFloatInstr(RVInstr):
    authorized_instr_strs = IntToFloatInstrs

    def __init__(
        self,
        instr_str: str,
        frd: FloatReg,
        rs1: IntReg,
        rm: int,
        is_rv64: bool,
        iscompressed: bool = False,
    ):
        self.rs1 = rs1
        self.frd = frd
        self.rm = rm
        super().__init__(instr_str, iscompressed)

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # rv32f
        if self.instr_str == "fcvt.s.w":
            return rv32f_fcvtsw(self.frd.value, self.rs1.value, self.rm)
        elif self.instr_str == "fcvt.s.wu":
            return rv32f_fcvtswu(self.frd.value, self.rs1.value, self.rm)
        # rv64f
        elif self.instr_str == "fcvt.s.l":
            return rv64f_fcvtsl(self.frd.value, self.rs1.value, self.rm)
        elif self.instr_str == "fcvt.s.lu":
            return rv64f_fcvtslu(self.frd.value, self.rs1.value, self.rm)
        # rv32d
        elif self.instr_str == "fcvt.d.w":
            return rv32d_fcvtdw(self.frd.value, self.rs1.value, self.rm)
        elif self.instr_str == "fcvt.d.wu":
            return rv32d_fcvtdwu(self.frd.value, self.rs1.value, self.rm)
        # rv64d
        elif self.instr_str == "fcvt.d.l":
            return rv64d_fcvtdl(self.frd.value, self.rs1.value, self.rm)
        elif self.instr_str == "fcvt.d.lu":
            return rv64d_fcvtdlu(self.frd.value, self.rs1.value, self.rm)
        # Default case
        else:
            raise ValueError(f"Unexpected instruction string: `{self.instr_str}`.")

    def get_inputregs(self) -> list[IntReg | FloatReg | CSR]:
        FRM_INSTR = (
            "fcvt.s.w",
            "fcvt.s.wu",
            "fcvt.s.l",
            "fcvt.s.lu",
            "fcvt.d.l",
            "fcvt.d.lu",
        )
        if self.rm == 0b111 and self.instr_str in FRM_INSTR:
            return [self.rs1, CSR.FRM]
        else:
            return [self.rs1]


# Pure float instructions with frs1, frs2, frs3 and frd
Float4Instrs = (
    "fmadd.s",
    "fmsub.s",
    "fnmsub.s",
    "fnmadd.s",
    "fmadd.d",
    "fmsub.d",
    "fnmsub.d",
    "fnmadd.d",
)


class Float4Instr(RVInstr):
    authorized_instr_strs = Float4Instrs

    def __init__(
        self,
        instr_str: str,
        frd: FloatReg,
        frs1: FloatReg,
        frs2: FloatReg,
        frs3: FloatReg,
        rm: int,
        is_rv64: bool,
        iscompressed: bool = False,
    ):
        self.rm = rm
        self.frs1 = frs1
        self.frs2 = frs2
        self.frs3 = frs3
        self.frd = frd
        super().__init__(instr_str, iscompressed)

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # rv32f
        if self.instr_str == "fmadd.s":
            return rv32f_fmadds(
                self.frd.value,
                self.frs1.value,
                self.frs2.value,
                self.frs3.value,
                self.rm,
            )
        elif self.instr_str == "fmsub.s":
            return rv32f_fmsubs(
                self.frd.value,
                self.frs1.value,
                self.frs2.value,
                self.frs3.value,
                self.rm,
            )
        elif self.instr_str == "fnmsub.s":
            return rv32f_fnmsubs(
                self.frd.value,
                self.frs1.value,
                self.frs2.value,
                self.frs3.value,
                self.rm,
            )
        elif self.instr_str == "fnmadd.s":
            return rv32f_fnmadds(
                self.frd.value,
                self.frs1.value,
                self.frs2.value,
                self.frs3.value,
                self.rm,
            )
        # rv32d
        elif self.instr_str == "fmadd.d":
            return rv32d_fmaddd(
                self.frd.value,
                self.frs1.value,
                self.frs2.value,
                self.frs3.value,
                self.rm,
            )
        elif self.instr_str == "fmsub.d":
            return rv32d_fmsubd(
                self.frd.value,
                self.frs1.value,
                self.frs2.value,
                self.frs3.value,
                self.rm,
            )
        elif self.instr_str == "fnmsub.d":
            return rv32d_fnmsubd(
                self.frd.value,
                self.frs1.value,
                self.frs2.value,
                self.frs3.value,
                self.rm,
            )
        elif self.instr_str == "fnmadd.d":
            return rv32d_fnmaddd(
                self.frd.value,
                self.frs1.value,
                self.frs2.value,
                self.frs3.value,
                self.rm,
            )
        # Default case
        else:
            raise ValueError(f"Unexpected instruction string: `{self.instr_str}`.")

    def get_inputregs(self) -> list[IntReg | FloatReg | CSR]:
        if self.rm == 0b111:
            return [self.frs1, self.frs2, self.frs3, CSR.FRM]
        else:
            return [self.frs1, self.frs2, self.frs3]


# Pure float instructions with frs1, frs2 and frd
Float3Instrs = (
    "fadd.s",
    "fsub.s",
    "fmul.s",
    "fdiv.s",
    "fadd.d",
    "fsub.d",
    "fmul.d",
    "fdiv.d",
)


class Float3Instr(RVInstr):
    authorized_instr_strs = Float3Instrs

    def __init__(
        self,
        instr_str: str,
        frd: FloatReg,
        frs1: FloatReg,
        frs2: FloatReg,
        rm: int,
        is_rv64: bool,
        iscompressed: bool = False,
    ):
        self.rm = rm
        self.frs1 = frs1
        self.frs2 = frs2
        self.frd = frd
        super().__init__(instr_str, iscompressed)

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # rv32f
        if self.instr_str == "fadd.s":
            return rv32f_fadds(
                self.frd.value, self.frs1.value, self.frs2.value, self.rm
            )
        elif self.instr_str == "fsub.s":
            return rv32f_fsubs(
                self.frd.value, self.frs1.value, self.frs2.value, self.rm
            )
        elif self.instr_str == "fmul.s":
            return rv32f_fmuls(
                self.frd.value, self.frs1.value, self.frs2.value, self.rm
            )
        elif self.instr_str == "fdiv.s":
            return rv32f_fdivs(
                self.frd.value, self.frs1.value, self.frs2.value, self.rm
            )
        # rv32d
        elif self.instr_str == "fadd.d":
            return rv32d_faddd(
                self.frd.value, self.frs1.value, self.frs2.value, self.rm
            )
        elif self.instr_str == "fsub.d":
            return rv32d_fsubd(
                self.frd.value, self.frs1.value, self.frs2.value, self.rm
            )
        elif self.instr_str == "fmul.d":
            return rv32d_fmuld(
                self.frd.value, self.frs1.value, self.frs2.value, self.rm
            )
        elif self.instr_str == "fdiv.d":
            return rv32d_fdivd(
                self.frd.value, self.frs1.value, self.frs2.value, self.rm
            )
        # Default case
        else:
            raise ValueError(f"Unexpected instruction string: `{self.instr_str}`.")

    def get_inputregs(self) -> list[IntReg | FloatReg | CSR]:
        if self.rm == 0b111:
            return [self.frs1, self.frs2, CSR.FRM]
        else:
            return [self.frs1, self.frs2]


# Pure float instructions with frs1, frs2 and frd
Float3NoRmInstrs = (
    "fsgnj.s",
    "fsgnjn.s",
    "fsgnjx.s",
    "fmin.s",
    "fmax.s",
    "fsgnj.d",
    "fsgnjn.d",
    "fsgnjx.d",
    "fmin.d",
    "fmax.d",
)


class Float3NoRmInstr(RVInstr):
    authorized_instr_strs = Float3NoRmInstrs

    def __init__(
        self,
        instr_str: str,
        frd: FloatReg,
        frs1: FloatReg,
        frs2: FloatReg,
        is_rv64: bool,
        iscompressed: bool = False,
    ):
        self.frs1 = frs1
        self.frs2 = frs2
        self.frd = frd
        super().__init__(instr_str, iscompressed)

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # rv32f
        if self.instr_str == "fsgnj.s":
            return rv32f_fsgnjs(self.frd.value, self.frs1.value, self.frs2.value)
        elif self.instr_str == "fsgnjn.s":
            return rv32f_fsgnjns(self.frd.value, self.frs1.value, self.frs2.value)
        elif self.instr_str == "fsgnjx.s":
            return rv32f_fsgnjxs(self.frd.value, self.frs1.value, self.frs2.value)
        elif self.instr_str == "fmin.s":
            return rv32f_fmins(self.frd.value, self.frs1.value, self.frs2.value)
        elif self.instr_str == "fmax.s":
            return rv32f_fmaxs(self.frd.value, self.frs1.value, self.frs2.value)
        # rv32d
        if self.instr_str == "fsgnj.d":
            return rv32d_fsgnjd(self.frd.value, self.frs1.value, self.frs2.value)
        elif self.instr_str == "fsgnjn.d":
            return rv32d_fsgnjnd(self.frd.value, self.frs1.value, self.frs2.value)
        elif self.instr_str == "fsgnjx.d":
            return rv32d_fsgnjxd(self.frd.value, self.frs1.value, self.frs2.value)
        elif self.instr_str == "fmin.d":
            return rv32d_fmind(self.frd.value, self.frs1.value, self.frs2.value)
        elif self.instr_str == "fmax.d":
            return rv32d_fmaxd(self.frd.value, self.frs1.value, self.frs2.value)
        # Default case
        else:
            raise ValueError(f"Unexpected instruction string: `{self.instr_str}`.")


# Pure float instructions with frs1, and frd
Float2Instrs = ("fsqrt.s", "fsqrt.d", "fcvt.d.s", "fcvt.s.d")


class Float2Instr(RVInstr):
    authorized_instr_strs = Float2Instrs

    def __init__(
        self,
        instr_str: str,
        frd: FloatReg,
        frs1: FloatReg,
        rm: int,
        is_rv64: bool,
        iscompressed: bool = False,
    ):
        self.rm = rm
        self.frs1 = frs1
        self.frd = frd
        super().__init__(instr_str, iscompressed)

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # rv32f
        if self.instr_str == "fsqrt.s":
            return rv32f_fsqrts(self.frd.value, self.frs1.value, self.rm)
        # rv32d
        elif self.instr_str == "fsqrt.d":
            return rv32d_fsqrtd(self.frd.value, self.frs1.value, self.rm)
        # rv32d
        elif self.instr_str == "fcvt.d.s":
            return rv32d_fcvtds(self.frd.value, self.frs1.value, self.rm)
        # rv32d
        elif self.instr_str == "fcvt.s.d":
            return rv32d_fcvtsd(self.frd.value, self.frs1.value, self.rm)
        # Default case
        else:
            raise ValueError(f"Unexpected instruction string: `{self.instr_str}`.")

    def get_inputregs(self) -> list[IntReg | FloatReg | CSR]:
        FRM_INSTR = ("fsqrt.s", "fsqrt.d", "fcvt.s.d")
        if self.rm == 0b111 and self.instr_str in FRM_INSTR:
            return [self.frs1, CSR.FRM]
        else:
            return [self.frs1]


# Flating point instructions of 2 source floats but an integer destination
FloatIntRd2Instrs = ("feq.s", "flt.s", "fle.s", "feq.d", "flt.d", "fle.d")


class FloatIntRd2Instr(RVInstr):
    authorized_instr_strs = FloatIntRd2Instrs

    def __init__(
        self,
        instr_str: str,
        rd: IntReg,
        frs1: FloatReg,
        frs2: FloatReg,
        is_rv64: bool,
        iscompressed: bool = False,
    ):
        self.frs1 = frs1
        self.frs2 = frs2
        self.rd = rd
        super().__init__(instr_str, iscompressed)

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # rv32f
        if self.instr_str == "feq.s":
            return rv32f_feqs(self.rd.value, self.frs1.value, self.frs2.value)
        elif self.instr_str == "flt.s":
            return rv32f_flts(self.rd.value, self.frs1.value, self.frs2.value)
        elif self.instr_str == "fle.s":
            return rv32f_fles(self.rd.value, self.frs1.value, self.frs2.value)
        # rv32d
        elif self.instr_str == "feq.d":
            return rv32d_feqd(self.rd.value, self.frs1.value, self.frs2.value)
        elif self.instr_str == "flt.d":
            return rv32d_fltd(self.rd.value, self.frs1.value, self.frs2.value)
        elif self.instr_str == "fle.d":
            return rv32d_fled(self.rd.value, self.frs1.value, self.frs2.value)
        # Default case
        else:
            raise ValueError(f"Unexpected instruction string: `{self.instr_str}`.")


# Flating point instructions of 1 source float but an integer destination
FloatIntRd1Instrs = ("fmv.x.w", "fclass.s", "fclass.d", "fmv.x.d")


class FloatIntRd1Instr(RVInstr):
    authorized_instr_strs = FloatIntRd1Instrs

    def __init__(
        self,
        instr_str: str,
        rd: IntReg,
        frs1: FloatReg,
        is_rv64: bool,
        iscompressed: bool = False,
    ):
        self.frs1 = frs1
        self.rd = rd
        super().__init__(instr_str, iscompressed)

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # rv32f
        if self.instr_str == "fmv.x.w":
            return rv32f_fmvxw(self.rd.value, self.frs1.value)
        elif self.instr_str == "fclass.s":
            return rv32f_fclasss(self.rd.value, self.frs1.value)
        # rv32d
        elif self.instr_str == "fclass.d":
            return rv32d_fclassd(self.rd.value, self.frs1.value)
        # rv64d
        elif self.instr_str == "fmv.x.d":
            return rv64d_fmvxd(self.rd.value, self.frs1.value)
        # Default case
        else:
            raise ValueError(f"Unexpected instruction string: `{self.instr_str}`.")


# Flating point instructions of 1 source int but a float destination
FloatIntRs1Instrs = ("fmv.w.x", "fmv.d.x")


class FloatIntRs1Instr(RVInstr):
    authorized_instr_strs = FloatIntRs1Instrs

    def __init__(
        self,
        instr_str: str,
        frd: FloatReg,
        rs1: IntReg,
        is_rv64: bool,
        iscompressed: bool = False,
    ):
        self.rs1 = rs1
        self.frd = frd
        super().__init__(instr_str, iscompressed)

    def gen_bytecode_int(self, is_spike_resolution: bool):
        # rv32f
        if self.instr_str == "fmv.w.x":
            return rv32f_fmvwx(self.frd.value, self.rs1.value)
        # rv64d
        elif self.instr_str == "fmv.d.x":
            return rv64d_fmvdx(self.frd.value, self.rs1.value)
        # Default case
        else:
            raise ValueError(f"Unexpected instruction string: `{self.instr_str}`.")
