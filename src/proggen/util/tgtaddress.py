# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only
from params import MAX_NUM_PICKABLE_FLOATREGS
from riscv import (
    PrivLvl,
    ILEN,
    ExceptionCause,
    FpuState,
)
from instgen import (
    is_placeholder,
    TrapInstr,
    TrapInstrWrapper,
    TvecWriterInstr,
    EPCWriterInstr,
    MxdelegWriterInstr,
    MisalignedMemInstr,
    PrivDescentInstr,
    JALRInstr,
    JALInstr,
    BranchInstr,
    IntLoadInstr,
    IntStoreInstr,
    FloatLoadInstr,
    FloatStoreInstr,
    AmoInstr,
    AmoStore,
)
from typing import TYPE_CHECKING, Optional

if TYPE_CHECKING:
    from ...states import CoreState


def gen_producer_id_to_tgtaddr(
    corestate: "CoreState", memop_addrs: list[int], final_bb_base_addr: int
) -> dict[int, int]:
    """Ducring generation of the test case, we generate some instruction
    knowing that a register is in the CONSUMED state and can generate a specific
    offset value. This function is responsible to crate a mapping from these
    producing registers to the value (address) they should produce.

    Args:
        corestate (CoreState): Core specific state of the fuzzer
        memop_addrs (list[int]): Addresses of memory operation (load/stores)
        final_bb_base_addr (int): Address of the final basic block

    Returns:
        dict[int, int]: Mapping from the ID of producing registers to their
            respective target addresses
    """

    index_in_memaddr_array = 0
    bb_start_addr_idx = 1
    interrupt_handler_start_addr_idx = 0
    producer_id_to_tgtaddr: dict[int, int] = dict()

    last_mtvec_write: Optional[TvecWriterInstr] = None
    last_stvec_write: Optional[TvecWriterInstr] = None
    last_mepc_write: Optional[EPCWriterInstr] = None
    last_sepc_write: Optional[EPCWriterInstr] = None

    instr_seq = corestate.basic_blocks

    assert len(corestate.basic_blocks) == len(corestate.bb_start_addrs)

    for bb_id, bb_instrlist in enumerate(instr_seq):
        for instr_id, instr in enumerate(bb_instrlist):
            ###
            # First check for instructions that do not have an instruction string, such as
            #  placeholder instructions or some CSR write instructions.
            ###
            if is_placeholder(instr):
                continue
            elif isinstance(instr, TvecWriterInstr):
                if instr.is_mtvec:
                    last_mtvec_write = instr
                else:
                    last_stvec_write = instr
            elif isinstance(instr, EPCWriterInstr):
                if instr.is_mepc:
                    last_mepc_write = instr
                else:
                    last_sepc_write = instr
            elif isinstance(instr, MxdelegWriterInstr):
                assert instr.producer_id != None
                if __debug__:
                    assert (
                        instr.producer_id == None
                        or not instr.producer_id in producer_id_to_tgtaddr
                    ), "producer_id {} already in producer_id_to_tgtaddr".format(
                        instr.producer_id
                    )
                # Avoid writing bit 32
                producer_id_to_tgtaddr[instr.producer_id] = (
                    instr.val_to_write_cpu ^ 0x80000000
                )

            # In case of a privilege descent instruction
            elif isinstance(instr, PrivDescentInstr):
                if instr.is_helper:
                    continue

                if instr.will_trap:
                    assert last_mtvec_write
                    prod_id = last_mtvec_write.producer_id
                else:
                    if instr.is_mret:
                        assert last_mepc_write
                        prod_id = last_mepc_write.producer_id
                    else:
                        assert last_sepc_write
                        prod_id = last_sepc_write.producer_id

                # Get the next bb's start address
                if bb_start_addr_idx == len(corestate.bb_start_addrs):
                    addr = final_bb_base_addr
                else:
                    addr = corestate.bb_start_addrs[bb_start_addr_idx]
                    bb_start_addr_idx += 1
                if __debug__:
                    assert prod_id
                    assert prod_id == -1 or not prod_id in producer_id_to_tgtaddr, (
                        f"producer_id {prod_id} already in producer_id_to_tgtaddr"
                    )

                assert prod_id != None
                producer_id_to_tgtaddr[prod_id] = addr

                if instr.will_trap:
                    last_mtvec_write = None
                else:
                    if instr.is_mret:
                        last_mepc_write = None
                    else:
                        last_sepc_write = None
            elif isinstance(instr, TrapInstr):
                handler_tvec_write = False

                if instr.is_mtvec:
                    assert last_mtvec_write
                    tvec_producer_id = last_mtvec_write.producer_id
                else:
                    assert last_stvec_write
                    tvec_producer_id = last_stvec_write.producer_id

                # Get the next bb's start address
                if isinstance(instr, TrapInstrWrapper) and instr.special_interrupt_bb:
                    addr = corestate.interrupt_handlers_start_addr[
                        interrupt_handler_start_addr_idx
                    ]
                    # Account for the extra TvecWriter the handler might add
                    for handler_instr in corestate.interrupt_handlers[
                        interrupt_handler_start_addr_idx
                    ]:
                        if isinstance(handler_instr, TvecWriterInstr):
                            handler_tvec_write = True
                            last_mtvec_write = handler_instr
                    interrupt_handler_start_addr_idx += 1
                elif bb_start_addr_idx == len(corestate.bb_start_addrs):
                    addr = final_bb_base_addr
                else:
                    addr = corestate.bb_start_addrs[bb_start_addr_idx]
                    bb_start_addr_idx += 1
                assert tvec_producer_id
                if __debug__:
                    assert (
                        tvec_producer_id == -1
                        or not tvec_producer_id in producer_id_to_tgtaddr
                    ), (
                        f"producer_id {tvec_producer_id} already in producer_id_to_tgtaddr"
                    )

                producer_id_to_tgtaddr[tvec_producer_id] = addr

                # Do not use twice the same tvec value because we want to jump
                # to a new basic block.
                if not handler_tvec_write:
                    if instr.is_mtvec:
                        last_mtvec_write = None
                    else:
                        last_stvec_write = None

                # Some exceptions also require their own produced register, not
                # only for tvec but also to make a targeted memory operation
                if instr.producer_id is not None:
                    del addr
                    if isinstance(instr, MisalignedMemInstr):
                        addr = instr.misaligned_addr
                    else:
                        raise Exception(
                            "We expected only MisalignedMemInstr to have a producer_id."
                        )

                    if __debug__:
                        assert (
                            instr.producer_id == None
                            or not instr.producer_id in producer_id_to_tgtaddr
                        ), "producer_id {} already in producer_id_to_tgtaddr".format(
                            instr.producer_id
                        )
                    producer_id_to_tgtaddr[instr.producer_id] = addr

            elif isinstance(instr, JALRInstr) and instr.producer_id is not None:
                if bb_start_addr_idx == len(corestate.bb_start_addrs):
                    addr = final_bb_base_addr
                else:
                    addr = corestate.bb_start_addrs[bb_start_addr_idx]
                    bb_start_addr_idx += 1
                assert instr.producer_id
                if __debug__:
                    assert (
                        instr.producer_id == None
                        or not instr.producer_id in producer_id_to_tgtaddr
                    ), "producer_id {} already in producer_id_to_tgtaddr".format(
                        instr.producer_id
                    )
                producer_id_to_tgtaddr[instr.producer_id] = addr

            elif (
                isinstance(
                    instr,
                    (
                        IntLoadInstr,
                        IntStoreInstr,
                        FloatLoadInstr,
                        FloatStoreInstr,
                        AmoInstr,
                        AmoStore,
                    ),
                )
                and instr.producer_id is not None
            ):
                if __debug__:
                    assert (
                        instr.producer_id == None
                        or not instr.producer_id in producer_id_to_tgtaddr
                    ), "producer_id {} already in producer_id_to_tgtaddr".format(
                        instr.producer_id
                    )
                assert instr.producer_id
                producer_id_to_tgtaddr[instr.producer_id] = memop_addrs[
                    index_in_memaddr_array
                ]
                index_in_memaddr_array += 1

            elif (isinstance(instr, JALInstr) and not instr.is_absolute) or (
                isinstance(instr, BranchInstr)
                and instr.plan_taken
                and not instr.fixed_opcode
            ):
                # If this is the last before the final block, we need to steer toward the final block.
                if bb_start_addr_idx == len(corestate.bb_start_addrs):
                    curr_addr = (
                        corestate.bb_start_addrs[bb_id] + instr_id * 4
                    )  # NO_COMPRESSED
                    instr.imm = final_bb_base_addr - curr_addr
                else:
                    bb_start_addr_idx += 1

    if corestate.hartstate.privlvl != PrivLvl.Machine:
        _handle_finalblock_exception(
            corestate,
            final_bb_base_addr,
            last_mtvec_write,
            last_stvec_write,
            producer_id_to_tgtaddr,
        )

    if __debug__:
        index_in_memaddr_array = len(memop_addrs)

    return producer_id_to_tgtaddr


def _handle_finalblock_exception(
    corestate: "CoreState",
    final_bb_base_addr: int,
    last_mtvec_write: Optional[TvecWriterInstr],
    last_stvec_write: Optional[TvecWriterInstr],
    producer_id_to_tgtaddr: dict[int, int],
):
    """If we could not steer to machine mode before the final block, an
    exception will be raised when the block writes to MSTATUS to turn on the
    FPU, an illegal instruction exception.BAsed on the scenario, we set the
    address of xTVEC to skip the instruction, or if the exception switchs to
    machine mode, to try again.

    Args:
        corestate (CoreState): Core specific state of the fuzzer
        final_bb_base_addr (int): Entry address of the final basic block
        last_mtvec_write (Optional[TvecWriterInstr]): last MTVEC write instruction
        last_stvec_write (Optional[TvecWriterInstr]): last STVEC write instruction
        producer_id_to_tgtaddr (dict[int, int]): Dictionary to transmit the
            target addresses to their respective producers

    Raises:
        ValueError: if we cannot properly handle the execption raised in the
            final block
    """

    illegal_instr_id = ExceptionCause.ID_ILLEGAL_INSTRUCTION.value
    is_illegal_instr_deleg = bool(
        (corestate.hartstate.medeleg >> illegal_instr_id) & 0b1
    )
    is_fpu_active = corestate.hartstate.mstatus_fs != FpuState.Off

    if not is_illegal_instr_deleg and not is_fpu_active:
        # Repeat final block, but in M-mode
        addr = final_bb_base_addr
    elif is_illegal_instr_deleg and not is_fpu_active:
        # Skip the freg dump
        addr = final_bb_base_addr + ILEN * (MAX_NUM_PICKABLE_FLOATREGS + 3)
    elif is_fpu_active:
        # skip enabling the FPU and dump
        addr = final_bb_base_addr + ILEN * 3
    else:
        raise ValueError("unknown state in the final bb")

    if is_illegal_instr_deleg:
        # we might now have an stvec
        if not last_stvec_write:
            raise ValueError("Cannot handle final block exception, no_stvec")
        assert last_stvec_write.producer_id
        stvec_producer_id = last_stvec_write.producer_id
        producer_id_to_tgtaddr[stvec_producer_id] = addr
    else:
        if not last_mtvec_write:
            raise ValueError("Cannot handle final block exception, no_mtvec")
        assert last_mtvec_write.producer_id
        mtvec_producer_id = last_mtvec_write.producer_id
        producer_id_to_tgtaddr[mtvec_producer_id] = addr
