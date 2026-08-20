# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


# This script is a helper for interacting with spike.
import runparams
from params import (
    TOTAL_NUM_INTREGS,
    MAX_NUM_PICKABLE_FLOATREGS,
    get_design_march_flags_nocompressed,
)
from riscv import FPREG_ABINAMES, ILEN, FloatReg
from instgen import WfiInstr
from pathlib import Path
from typing import Optional, TYPE_CHECKING
import os
import subprocess
import re

if TYPE_CHECKING:
    from proggen import TestCaseGenerator
    from params import TestParams
    from riscv import IntReg


##
# Spike constants
##

SPIKE_STARTADDR = 0x80000000
SPIKE_MEDELEG_MASK = 0xCB3FF
SPIKE_MIDELEG_MASK = 0x222

###
# Helper functions
###


def _gen_presim_dbgcmdfile(
    identifier_str: str,
    startpc: int,
    regdump_reqs: list[tuple[int, bool, "IntReg"]],
    dump_final_reg_vals: bool,
    final_addr: int,
    hartid: int,
    dump_freg_format: str = "",
) -> str:
    """
    Generates the spike debug command file for tracing the registers at specific
    PC locations. This is useful to retrieve the value of dependant registers to
    entangle the control and data flow in the ultimate elf. This is what enable
    us to do the asymmetric ISA pre-simmulation

    Parameters:
        identifier_str: the identifier string
        startpc: the start PC
        regdump_reqs: an ordered list of tuples (pc_to_req, is_float_req, reg_to_dump)
        dump_final_reg_vals: whether to dump the final register values
        final_addr: the final address
        num_fp_regs: the number of floating point registers
        dump_freg_format: either '' or 'd' for 'fregd' or 's' for 'fregs'

    Returns:
        the path to the debug command file
    """
    # This assertion is to check whether we actually can remove dump_freg_format.
    assert not dump_freg_format

    path_to_debug_file = os.path.join(
        runparams.PATH_TO_TMP,
        "dbgcmds",
        f"cmds_trace_regs_at_pc_locs_{identifier_str}_hart{hartid}",
    )
    # if not os.path.exists(path_to_debug_file):
    Path(os.path.dirname(path_to_debug_file)).mkdir(parents=True, exist_ok=True)
    spike_debug_commands = [f"until pc {hartid} 0x{startpc:x}"]
    # Just make sure it won't coincide with the first requested pc.
    prev_req_pc = -1
    for next_pc, is_float_req, reg_to_dump in regdump_reqs:
        # The surrounding condition on permits to get multiple registers in the
        # same PC, without issuing an useless `until`. This is useful for
        # non-taken branches, for example, in which we need to know the values
        # of both operands.
        if prev_req_pc != next_pc:
            spike_debug_commands.append(f"until pc {hartid} 0x{startpc + next_pc:x}")
            prev_req_pc = next_pc
        spike_debug_commands.append(
            f"{'f' if is_float_req else ''}reg {hartid} {reg_to_dump.value}"
        )
    if dump_final_reg_vals:
        spike_debug_commands.append(f"until pc {hartid} 0x{final_addr:x}")
        spike_debug_commands.append(f"reg {hartid}")
        floatregs = list(FloatReg)
        for fp_reg in floatregs:
            spike_debug_commands.append(f"freg {hartid} {FPREG_ABINAMES[fp_reg.value]}")

    spike_debug_commands.append("q\n")
    spike_debug_commands_str = "\n".join(spike_debug_commands)

    with open(path_to_debug_file, "w") as f:
        f.write(spike_debug_commands_str)

    return path_to_debug_file


# Parse the hex values from ..a byte sequence
def _get_hex_val(dumped_val: bytes, truncate: int = 0) -> list[int]:
    """Finds the hex values in a byte sequence and returns them as integers."""
    dumped_val_str = dumped_val.decode("ascii")
    hex_values = re.findall(r"0x([0-9a-fA-F]+)", dumped_val_str)
    if truncate > 0:
        for i, val in enumerate(hex_values):
            hex_values[i] = val[truncate:]
    int_values = [int(val, 16) for val in hex_values]
    return int_values


def _run_spike_with_debugcmd(
    elfpath: str,
    rvflags: str,
    path_to_debug_file: str,
    identifier_str: str,
    hartcnt: int,
    spike_timeout: float,
) -> list[bytes]:
    """
    Runs an elf on spike, with a debug command file.

    Parameters:
        elfpath: path to the elf file
        rvflags: the riscv flags
        startpc: the start PC
        path_to_debug_file: the path to the debug command file
        identifier_str: the identifier string

    Returns:
        the output of the spike command

    """
    allowed_prefixes = [
        b"0x",
        b"zero:",
        b"tp:",
        b"s0:",
        b"a2:",
        b"a6:",
        b"s4:",
        b"s8:",
        b"t3:",
        b"core",
    ]

    spike_shell_command = (
        "spike",
        "-d",
        f"-p{hartcnt}",
        f"--debug-cmd={path_to_debug_file}",
        f"--isa={rvflags}",
        elfpath,
    )
    try:
        spike_out = subprocess.run(
            spike_shell_command,
            capture_output=True,
            timeout=spike_timeout,
        ).stderr
    except Exception:
        raise Exception(
            f"Spike timeout (A) for identifier str:\n{identifier_str}. \nCommand: {' '.join(filter(lambda s: '--debug-cmd' not in s, spike_shell_command))}\nDebug file: {path_to_debug_file}"
        )

    spike_out = spike_out.split(b"\n")
    spike_out = list(entry.strip() for entry in spike_out)
    spike_out = list(
        filter(
            lambda s: any(s.startswith(prefix) for prefix in allowed_prefixes),
            spike_out,
        )
    )
    spike_out = list(
        filter(lambda s: b"exception" not in s and b"tval 0x" not in s, spike_out)
    )
    spike_out = list(filter(lambda s: b">>>>" not in s, spike_out))

    if runparams.REMOVE_TMPFILES:
        os.remove(path_to_debug_file)

    return spike_out


def _parse_regdumps(
    spike_out: list[bytes],
    fp_regs: bool,
    has_fpdouble_support: bool,
    dump_freg_format: Optional[str] = None,
) -> tuple[list, list, list]:
    """Gets the register values from ..the spike output."""
    final_fpureg_vals = []
    final_fpureg_archvals = []
    final_intreg_vals = []
    while not spike_out[0].startswith(b"0x"):
        dumped_val = spike_out.pop(0)
        final_intreg_vals += _get_hex_val(dumped_val)
    assert len(final_intreg_vals) == TOTAL_NUM_INTREGS

    if fp_regs:
        for dumped_val in spike_out:
            if has_fpdouble_support:
                final_fpureg_vals += _get_hex_val(dumped_val, 16)
            else:
                final_fpureg_vals += _get_hex_val(dumped_val, 24)
            if dump_freg_format != "":
                final_fpureg_archvals.append(dumped_val)

    assert len(final_fpureg_vals) == MAX_NUM_PICKABLE_FLOATREGS

    return (final_intreg_vals, final_fpureg_vals, final_fpureg_archvals)


###
# Exposed functions
###


# @brief runs and traces every PC location.
# @param regdump_reqs: see _gen_presim_dbgcmdfile
# @param dump_freg_format: either '' or 'd' for 'fregd' or 's' for 'fregs'
# @return list of register values. If dump_final_reg_vals is True, then the
# output is a pair, whose second element is a pair of array of final register
# values, for int and float registers
def asymmetric_isa_presim(
    test_params: "TestParams",
    elfpath: str,
    regdump_reqs,
    final_addr: int,
    hartid: int,
    dump_final_reg_vals: bool = True,
    dump_freg_format: str = "",
) -> tuple[list, tuple[list, list, list]]:
    """
    Runs the elf on spike and traces the register values at specific PC locations
    to entangle the control and data flow in the ultimate elf.
    Spike output:
    b'0xffffffffffff0003'

    Parameters:
        identifier_str: the identifier string
        elfpath: the path to the elf file
        rvflags: the riscv flags
        regdump_reqs: an ordered list of tuples (pc_to_req, is_float_req, reg_to_dump)
        dump_final_reg_vals: whether to dump the final register values
        final_addr: the final address
        num_fp_regs: the number of floating point registers
        has_fpdouble_support: whether the design supports double precision
        dump_freg_format: either '' or 'd' for 'fregd' or 's' for 'fregs'

    Returns:
        list of register values. If dump_final_reg_vals is True, then the
        output is a pair, whose second element is a pair of array of final register
        values, for int and float registers
    """
    identifier_str = test_params.instance_to_str()
    fp_regs = test_params.design_has_fpu
    has_fpdouble_support = test_params.design_has_fpud
    rvflags = get_design_march_flags_nocompressed(test_params.design_name)
    spike_timeout = test_params.spike_timeout
    hartcnt = test_params.num_harts

    path_to_debug_file = _gen_presim_dbgcmdfile(
        identifier_str,
        SPIKE_STARTADDR,
        regdump_reqs,
        dump_final_reg_vals,
        final_addr,
        hartid,
        dump_freg_format,
    )

    spike_out = _run_spike_with_debugcmd(
        elfpath, rvflags, path_to_debug_file, identifier_str, hartcnt, spike_timeout
    )

    ret = []
    regsdump = ([], [], [])
    while spike_out and (not spike_out[0].startswith(b"zero:")):
        dumped_val = spike_out.pop(0)
        if dumped_val.startswith(b"0x"):
            ret += _get_hex_val(dumped_val)
        elif dumped_val in ("M", "S", "U"):
            ret.append(chr(dumped_val))
        else:
            print(dumped_val)
            raise NotImplementedError(f"Line not supported: {dumped_val}")

    # Potentially get the final register values
    if dump_final_reg_vals:
        assert spike_out, f"{spike_out}, {ret}, {test_params.instance_to_str()}"
        regsdump = _parse_regdumps(
            spike_out, fp_regs, has_fpdouble_support, dump_freg_format
        )
    else:
        assert not spike_out

    # Return the values
    return ret, regsdump


def verify_pc_trace(
    identifier_str: str,
    elfpath: str,
    rvflags: str,
    testcase_generator: "TestCaseGenerator",
    spike_timeout: float,
):
    """Used for debugging, should never be used in releasse, use -O then"""
    hartcnt = len(testcase_generator.corestates)

    spike_shell_command = (
        "spike",
        "-g",
        f"-p{hartcnt}",
        f"--isa={rvflags}",
        elfpath,
    )

    try:
        spike_out = subprocess.run(
            spike_shell_command,
            capture_output=True,
            timeout=spike_timeout,
        ).stderr
    except Exception as e:
        cmd = {" ".join(filter(lambda s: "--debug-cmd" not in s, spike_shell_command))}
        raise Exception(
            "Spike timeout (A) for identifier str: \n{}\nCommand: {}".format(
                identifier_str, cmd
            )
        )

    # FIXME unseen instructions not well handled
    return

    spike_out_str = spike_out.decode("utf-8")
    pcs_and_freq = spike_out_str.split("\n")
    # Filter out all that is not (hex decimal), and not starting with 0x8...
    pcs: list[int] = []
    pattern = r"^8[0-9a-fA-F]+ \d+$"
    for line in pcs_and_freq:
        if re.match(pattern, line):
            pc = int(line.split()[0], 16)
            # freq = int(line.split()[1])
            if pc not in pcs:
                pcs.append(pc)

    n_match = 0
    fuzzerstate = testcase_generator.fuzzerstate
    base_addr = fuzzerstate.init_bb_base_addr
    for offset in range(len(fuzzerstate.init_bb_instrs)):
        addr = base_addr + offset * ILEN + SPIKE_STARTADDR
        if addr in pcs:
            n_match += 1
        else:
            raise ValueError("PC sequence did not match our expecations")

    base_addr = fuzzerstate.get_final_bb_base_addr()
    for offset in range(len(fuzzerstate.final_bb_instrs)):
        # FIXME certain design will actuall exectue WFI and the following jump
        if isinstance(fuzzerstate.final_bb_instrs[offset], WfiInstr):
            continue
        if offset > 1 and isinstance(fuzzerstate.final_bb_instrs[offset - 1], WfiInstr):
            continue
        addr = base_addr + offset * ILEN + SPIKE_STARTADDR
        if addr in pcs:
            n_match += 1
        else:
            raise ValueError("PC sequence did not match our expectations")

    for core in testcase_generator.corestates.values():
        for bb_base_addr, bb_instrs in zip(core.bb_start_addrs, core.basic_blocks):
            for offset in range(len(bb_instrs)):
                # FIXME certain design will actuall exectue WFI and the following jump
                if isinstance(bb_instrs[offset], WfiInstr):
                    continue
                if offset > 1 and isinstance(bb_instrs[offset - 1], WfiInstr):
                    continue
                addr = bb_base_addr + offset * ILEN + SPIKE_STARTADDR
                if addr in pcs:
                    n_match += 1
                else:
                    raise ValueError("PC sequence did not match our expecations")

    # We could also verify frequency
    assert n_match == len(pcs), f"matched with {n_match} on {len(pcs)} addr"
