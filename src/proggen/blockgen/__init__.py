# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


from .basicblock import gen_basicblock, alloc_next_bb_addr
from .stateloop import gen_minimal_interrupt_handler, gen_state_preserving_loop
