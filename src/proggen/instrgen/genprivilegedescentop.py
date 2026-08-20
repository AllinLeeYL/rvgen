# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


# This script is used to pick an instruction from ..the privileged descent instruction ISA class.

from riscv import PrivLvl
from instgen import PrivDescentInstr
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from states import CoreState, HartState


def gen_priv_descent_instr(corestate: "CoreState"):
    """
    Generate a priviledge descent instruction, either mRET or sRET
    """
    hartstate: "HartState" = corestate.hartstate

    will_trap = (
        hartstate.privlvl == PrivLvl.Machine
        and corestate.hartstate.mip_msip
        and corestate.hartstate.mie_msie
        and (
            corestate.hartstate.mstatus_mpp != PrivLvl.Machine
            or (
                corestate.hartstate.mstatus_mpp == PrivLvl.Machine
                and corestate.hartstate.mstatus_mpie
            )
        )
    )

    if __debug__:
        if hartstate.privlvl == PrivLvl.Machine:
            # Add `or hartstate.is_sepc_populated` to implement sret in machine
            # mode
            assert hartstate.is_mepc_populated != None or will_trap, (
                "MEPC or SEPC must be populated to descend privileges from M-mode"
            )
            assert hartstate.mstatus_mpp != None, (
                "MPP must be populated to descend privileges from M-mode."
            )
        else:
            assert hartstate.privlvl == PrivLvl.Supervisor
            assert hartstate.is_sepc_populated != None, (
                "In S-mode, SEPC must be populated to descend privileges."
            )
            assert hartstate.mstatus_spp != None, (
                "SPP must be populated to descend privileges from S-mode."
            )

    is_mret = hartstate.privlvl == PrivLvl.Machine
    old_privilege = hartstate.privlvl
    assert corestate.hartstate.mstatus_mdt == False
    # If the priviledge descent causes a trap
    if will_trap:
        assert corestate.hartstate.is_mtvec_populated
        corestate.hartstate.is_mepc_populated = False
        hartstate.is_mtvec_populated = False
        hartstate.mstatus_mie = False
        hartstate.mstatus_mpie = True
        # MPP take the value of itself, since the switch happens before
        return PrivDescentInstr(is_mret, old_privilege, will_trap=True)

    if is_mret:
        assert corestate.hartstate.is_mepc_populated
        hartstate.is_mepc_populated = False
        hartstate.privlvl = hartstate.mstatus_mpp
        hartstate.mstatus_mpp = PrivLvl.User
        hartstate.mstatus_mie = hartstate.mstatus_mpie
        hartstate.mstatus_mpie = True
    else:
        hartstate.is_sepc_populated = False
        hartstate.privlvl = hartstate.mstatus_spp
        hartstate.mstatus_spp = PrivLvl.User

    return PrivDescentInstr(is_mret, old_privilege)
