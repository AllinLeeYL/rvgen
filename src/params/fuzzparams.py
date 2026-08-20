# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


from riscv import IntReg, ILEN, DWORD_SIZE
import runparams
##
# Tunable parameters
##

MAX_NUM_STORE_LOCATIONS = 5
# MAX_N_CONSECUTIVE_MEMOPS = 31 - MAX_NUM_STORE_LOCATIONS - 2
BRANCH_TAKEN_PROBA = 0.3
# Stop generating instructions when the memory saturation reaches this level.
# In other words, if the memory is occupied by more than this amount, then
# do not start generating new basic blocks.
LIMIT_MEM_SATURATION_RATIO = 0.8
# When a register is produced, it gets this probability to be picked next.
# What is nice is that it immediately saturates: producing it twice does not
# increase picking proba.
REGPICK_PROTUBERANCE_RATIO = 0.2
SIMPLE_ILLEGAL_INSTRUCTION_PROBA = 0.01
# Having this being zero eases the analysis of the program since we can try to
# simply remove all the FPU activations/deactivations to ensure that dumping is
# possible. More sophisticated methods could be implemented.
PROBA_PICK_WRONG_FPU = 0.0
PROBA_AUTHORIZE_PRIVILEGES = 0.05

# We prioritize finishing the production of a register
REG_FSM_WEIGHTS = [
    1,  # FREE -> PRODUCED0
    10,  # PRODUCED0 -> PRODUCED1
    10,  # PRODUCED1 -> FREE/CONSUMED
]

##
# Constants
##

# Address outside the reachable address space. WARNING, this is intialized by
# the elf template and should always mirror the elf template
DATA_SECTION_BASE = 0x80180000
STRESS_SECTION_BASE = 0x80140000
# semaphore-ish addr for the last block
SEMAPHORE_1 = DATA_SECTION_BASE  # [0x80100000 - 0x80100007]
# barrier for the MCM reset handler
BARRIER_COUNT = DATA_SECTION_BASE + DWORD_SIZE  # [0x80100008 - 0x8010000f]
BARRIER_PHASE = DATA_SECTION_BASE + 2 * DWORD_SIZE  # [0x80100010 - 0x80100018]
# core synchronization addresses
CORESYNC_ADDR_BASE = DATA_SECTION_BASE + 3 * DWORD_SIZE  # Up to 1023 hart, CLINT
# barrier, offset to the sync addr base
INTERRUPT_BARRIERS_OFFSET = 0x0
INTERRUPT_BARRIER_LEN = 0x100 # supports 256 barriers (harts)
MCM_BARRIERS_OFFSET = INTERRUPT_BARRIER_LEN
MCM_BARRIER_LEN = 0x100 # supports 256 barriers (harts)
INSTR_BARRIERS_OFFSET = INTERRUPT_BARRIER_LEN + MCM_BARRIER_LEN
INSTR_BARRIER_LEN = 0x100 # supports 256 barriers (harts)

MCM_RESET_HANDLER_LEN = 0x1000  # [memsize - memsize + 0x1000]

# simulation termination variable
TOHOST_ADDR = 0x80200000  # [0x80200000 - 0x80200007]
TERMSIG = 0x1

# Give some room for the bootloader
SETUP_CYCLES = 1000
EXTRA_TOHOST_INSTR = 400
MAX_CYCLES_PER_INSTR = 100

# Basic blocks
BASIC_BLOCK_MIN_SPACE = ILEN * 13
MAX_STATE_PRESERVING_LOOP_INSTR = 28

# There should always be at least this number of free registers
NUM_MIN_INPUTS = 4  # Including 0 reg
NUM_MIN_OUTPUTS = 2  # Including 0 reg

# We need at least 4 (+ the zero reg) registers to entangle the data flow, and
# registers x26-x31 are reserved as helpers for the fuzzer
MIN_NUM_PICKABLE_INTREGS = 6
MIN_NUM_PICKABLE_FLOATREGS = 6
MAX_NUM_PICKABLE_INTREGS = 24  # We leave at least one reg for load/store address
MAX_NUM_PICKABLE_FLOATREGS = 32  # We could reduce that
TOTAL_NUM_INTREGS = 32

##
# Registers with special use cases for the fuzzer
##

# Holds the base address of the current design
# WARNING this mask is also used as the destination when dumping register values
RELOCATOR_REG = (IntReg.t6, 0x80000000)
# Mask to limit the value of the dependent registers (use by pre-consumer)
RDEP_MASK_REG = (IntReg.t5, 0xFFFFFFFF)
# Mask to enable or disable the FPU
FPU_STATE_BITS_REG = (IntReg.t4, 0x6000)
# Mask to switch both MPP bits
MPP_BOTH_BITS_REG = (IntReg.t3, 0x1800)
# Used to store the current pc, to jump back from the handler
MCM_RESET_REG = IntReg.s11
# Mask to sync with other core
CORESYNC_ADDR_REG = (IntReg.s10, CORESYNC_ADDR_BASE)

# Mask to switch the (unique) SPP Bit
SPP_BIT_VAL = 0x100
MPIE_BIT_VAL = 0x80

##
# Basic assertions
##

assert RELOCATOR_REG[0].value < 32
assert RDEP_MASK_REG[0].value < 32
assert FPU_STATE_BITS_REG[0].value < 32
assert MPP_BOTH_BITS_REG[0].value < 32
assert MCM_RESET_REG.value < 32
assert RELOCATOR_REG[0].value >= MAX_NUM_PICKABLE_INTREGS
assert RDEP_MASK_REG[0].value >= MAX_NUM_PICKABLE_INTREGS
assert FPU_STATE_BITS_REG[0].value >= MAX_NUM_PICKABLE_INTREGS
assert MPP_BOTH_BITS_REG[0].value >= MAX_NUM_PICKABLE_INTREGS
assert MCM_RESET_REG.value >= MAX_NUM_PICKABLE_INTREGS
