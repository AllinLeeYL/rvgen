# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only
import os

# Module to hold global vraibles that can be set by the main script and used
# throughout the project. Therefore this module should be imported and used
# mutably

PATH_TO_TMP = os.path.join(".")
os.makedirs(PATH_TO_TMP, exist_ok=True)
# elf dir
elfs_dir = os.path.join(PATH_TO_TMP, "elfs")
os.makedirs(elfs_dir, exist_ok=True)
# litmus dir
litmus_dir = os.path.join(PATH_TO_TMP, "litmus")
os.makedirs(litmus_dir, exist_ok=True)

# Used for debugging purposes.
REMOVE_TMPFILES = True

# Skip testcase without interrupts
SKIP_NO_INTERRUPT = False

# Option to keep the CLINT address of spike
DEBUG_RTL = False

# Disables Memory Consistency Fuzzing when set to False
FUZZ_MCM = False

# Enables stress core generation
FUZZ_STRESS_CORE = True

# Use new memory operation address generation scheme
USE_ADDR_REGS = False

# skips the final register check
NO_FINAL_CHECK = False

# enanbles interrupts
FUZZ_INTERRUPT = False

# verifies the memory trace instead of enumerating, backend can be herd or dartagnan
VERIFY_MEMORY_TRACE = None

# verifies the memory trace instead of enumerating
JIT_SCENARIO = False

# Maximum number of consecutive memory operations per basic block
MAX_N_CONSECUTIVE_MEMOPS = 5
