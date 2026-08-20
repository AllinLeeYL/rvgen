# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


from riscv import ExceptionCause
from params.config_loader import get_instruction_weights, get_exception_weights

# Load configuration from TOML file
# These can be overridden by setting the HARTATTACK_CONFIG environment variable
# to point to a custom configuration file

ISAINSTRCLASS_INITIAL_BOOSTERS = get_instruction_weights()
EXCEPTION_OP_TYPE_INITIAL_BOOSTERS = get_exception_weights()
