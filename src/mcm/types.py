# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


from typing import Tuple

HartId = int
Addr = int
InstrCoord = Tuple[int, int]
StepRange = Tuple[int, int]
MemOrdering = list[InstrCoord]
PoIdx = int
Node = tuple[HartId, PoIdx]
