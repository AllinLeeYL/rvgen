# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only



# This function sets the value of the given register using an lui+addi sequence.
# @param do_check_bounds if True, will check that the value is not too big to
# fit in 31 bits (i.e., in 32 bits but without being sign-extended to 64 bits).
# @return pair (lui_imm: int, addi_imm: int)
def li_into_reg(val_unsigned: int, do_check_bounds: bool = True):
    if do_check_bounds:
        assert val_unsigned >= 0
        assert val_unsigned < 0x80000000, (
            f"For the destination address `{hex(val_unsigned)}`, we will need to manage sign extension, which is not yet implemented here."
        )

    # Check whether the MSB of the addi would be 1. In this case, we will add 1 to the lui
    is_sign_extend_ones = (val_unsigned >> 11) & 1
    addi_imm = val_unsigned & 0xFFF
    # Make it negative properly
    if is_sign_extend_ones:
        addi_imm = -((~addi_imm) & 0xFFF) - 1
    lui_imm = (int(is_sign_extend_ones) + (val_unsigned >> 12)) & 0xFFFFF
    return lui_imm, addi_imm


# from ..an unsigned int coded on 32 or 64 bits, returns the signed value when interpreting the value as signed
def twos_complement(val_unsigned: int, is_rv64: bool):
    if is_rv64:
        if __debug__:
            assert val_unsigned >= 0
            assert val_unsigned < 1 << 64
        return val_unsigned - (((val_unsigned >> 63) & 1) << 64)
    else:
        if __debug__:
            assert val_unsigned >= 0
            assert val_unsigned < 1 << 32
        return val_unsigned - (((val_unsigned >> 31) & 1) << 32)


# from ..a signed int, returns an unsigned version.
# This should be a reciprocal function of twos_complement.
def to_unsigned(val_signed: int, is_rv64: bool):
    if is_rv64:
        if __debug__:
            assert val_signed < 1 << 63
        return (((val_signed >> 63) & 1) << 64) + val_signed
    else:
        if __debug__:
            assert val_signed < 1 << 32
        return (((val_signed >> 31) & 1) << 32) + val_signed
