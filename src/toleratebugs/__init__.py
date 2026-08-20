# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


from .toleratebugs import (
    tolerate_bug_for_eval_reduction,
    tolerate_bug_for_bug_timing,
    is_forbid_vexriscv_csrs,
    is_tolerate_rocket_minstret,
    is_tolerate_kronos_readbadcsr,
    is_tolerate_picorv32_readnonimplcsr,
    is_tolerate_vexriscv_fpu_disabled,
    is_tolerate_vexriscv_fpu_leak,
    is_tolerate_cva6_fdivs_flags,
    is_tolerate_vexriscv_imprecise_fcvt,
    is_tolerate_vexriscv_fmin,
    is_tolerate_vexriscv_double_to_float,
    is_tolerate_vexriscv_dependent_single_precision,
    is_tolerate_vexriscv_dependent_fle_feq_ret1,
    is_tolerate_vexriscv_dependent_flt_ret0,
    is_tolerate_vexriscv_sqrt,
    is_tolerate_vexriscv_muldiv_conversion,
    is_tolerate_cva6_single_precision,
    is_tolerate_cva6_division,
    is_tolerate_kronos_fence,
    is_tolerate_picorv32_fence,
    is_tolerate_picorv32_missingmandatorycsrs,
    is_tolerate_picorv32_readhpm_nocsrrs,
    is_no_interaction_minstret,
    is_tolerate_kronos_minstret,
    is_tolerate_vexriscv_minstret,
    is_tolerate_picorv32_writehpm,
    is_tolerate_cva6_mhpmcounter,
    is_tolerate_boom_minstret,
    is_tolerate_vexriscv_mhpmcountern,
    is_tolerate_cva6_mhpmevent31,
    is_tolerate_naxriscv_clint,
)
