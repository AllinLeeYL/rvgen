# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


MAX_DEVICES = 1023
MAX_HARTS = 15872
PRIORITY_BASE = 0x0
PENDING_BASE = 0x1000
ENABLE_BASE = 0x2000
HART_BASE = 0x200000

# 1 << 25
MSIP_WIDTH = 0x4
MTIMER_OFFSET = 0x4000
SPIKE_CLINT_BASE = 0x2000000
