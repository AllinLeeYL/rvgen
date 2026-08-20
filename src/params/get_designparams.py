# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


from functools import cache
import json
import os


# @return the path to the design simulation binary
@cache
def get_design_simulator(design_name):
    with open(os.path.join("cfg.json"), "r") as f:
        cfg = json.load(f)
    simulator = cfg["simname"]
    simulator_bin = os.path.join(simulator)
    return simulator_bin


# @param design_name: must be one of the keys of the design_repos.json dict.
# @return the design config of the relevant repo.
@cache
def get_design_cfg(design_name):
    with open(os.path.join("cfg.json"), "r") as f:
        return json.load(f)


@cache
def get_design_boot_addr(design_name) -> int:
    device_config = get_design_cfg(design_name)
    return int(device_config["bootaddr"], base=0)


@cache
def get_design_medelg_mask(design_name) -> int:
    """returns the medeleg mask of the design"""
    device_config = get_design_cfg(design_name)
    return int(device_config["medeleg"], base=16)


@cache
def get_design_extentions(design_name: str) -> list[str]:
    """Returns the extensions supported by the design."""
    device_config = get_design_cfg(design_name)
    return list(device_config["rvext"])


@cache
def get_design_midelg_mask(design_name) -> int:
    """returns the mideleg mask of the design"""
    device_config = get_design_cfg(design_name)
    return int(device_config["mideleg"], base=16)


@cache
def get_design_clint_addr(design_name) -> int:
    """returns the clint address of the design"""
    device_config = get_design_cfg(design_name)
    return int(device_config["clint"], base=16)


# @param design_name: must be one of the keys of the design_repos.json dict.
# @return the march flags for the design, for example `-march=rv64gc -mabi=lp64`.
def get_design_march_ccflags(design_name) -> str:
    device_config = get_design_cfg(design_name)
    return device_config["marchflags"]


# @param design_name: must be one of the keys of the design_repos.json dict.
# @return for example `rv64gc`.
def get_design_march_flags(design_name) -> str:
    return (
        get_design_march_ccflags(design_name).split("-march=")[1].split(" ")[0].lower()
    )


@cache
def get_design_hartids(design_name) -> list[int]:
    """Return the number of cores used by the design
    Args:
        design_name (str): name of the design

    Returns:
        list[int]: a list of all hart ids
    """
    device_config = get_design_cfg(design_name)
    hartids = device_config["hartids"]
    return hartids


@cache
def wfi_u_mode_avail(design_name) -> bool:
    """Checks if a design can use the WFI in U-mode
    Args:
        design_name (str): name of the design

    Returns:
        bool: True is WFI is available in U-mode
    """
    device_config = get_design_cfg(design_name)
    wfi_u_mode = device_config["wfi_u_mode"]
    return wfi_u_mode


@cache
def get_simulator(design_name) -> str:
    device_config = get_design_cfg(design_name)
    simulator = device_config["simulator"]
    return simulator


def get_design_march_flags_nocompressed(design_name) -> str:
    return (
        get_design_march_ccflags(design_name)
        .split("-march=")[1]
        .split(" ")[0]
        .lower()
        .replace("c", "")
    )


# @param design_name: must be one of the keys of the design_repos.json dict.
# @return true iff the design is 32bit.
@cache
def is_design_32bit(design_name) -> bool:
    march_flags = get_design_march_flags(design_name)
    assert "128" not in march_flags, (
        "Ensure that you support 128-bit design everywhere, and then remove this assertion."
    )
    return "32" in march_flags


@cache
def f_ext_support(design_name) -> bool:
    return "f" in get_design_march_flags(design_name) or "g" in get_design_march_flags(
        design_name
    )


@cache
def d_ext_support(design_name) -> bool:
    return "d" in get_design_march_flags(design_name) or "g" in get_design_march_flags(
        design_name
    )


@cache
def m_ext_support(design_name) -> bool:
    return "m" in get_design_march_flags(design_name) or "g" in get_design_march_flags(
        design_name
    )


@cache
def a_ext_support(design_name) -> bool:
    return "a" in get_design_march_flags(design_name) or "g" in get_design_march_flags(
        design_name
    )


@cache
def c_ext_support(design_name: str) -> bool:
    return "c" in get_design_march_flags(design_name)


def misaligned_data_support(design_name) -> bool:
    """Gets the misaligned data support param of the design
    Args:
        design_name (str): name of the design

    Returns:
        bool: true if the design support misalligned data access
    """
    device_config = get_design_cfg(design_name)
    return device_config["misaligned_data_supported"]


def design_get_privlvs_letters(design_name) -> str:
    """Gets the priviledge mode supported by the design

    Args:
        design_name (str): name of the design

    Returns:
        str: "u" for user, "s" for supervisor, "m" for machine ("msu" for all)
    """
    device_config = get_design_cfg(design_name)
    return device_config["privlvs"]


def s_mode_support(design_name) -> bool:
    return "s" in design_get_privlvs_letters(design_name)


def u_mode_support(design_name) -> bool:
    return "u" in design_get_privlvs_letters(design_name)


# MMU
def design_has_only_bare(design_name) -> bool:
    return not get_design_cfg(design_name)["mmu"]


def design_has_sv32(design_name) -> bool:
    return "sv32" in get_design_cfg(design_name)["mmu"]


def design_has_sv39(design_name) -> bool:
    return "sv39" in get_design_cfg(design_name)["mmu"]


def design_has_sv48(design_name) -> bool:
    return "sv48" in get_design_cfg(design_name)["mmu"]


# FIXME add to cfg file
def pmp_support(design_name) -> bool:
    if design_name in ("picorv32", "toooba", "toooba-3core", "naxriscv", "vexiiriscv"):
        return False
    return True
