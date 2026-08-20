# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


import os
import toml
from pathlib import Path
from typing import Dict, Any

from riscv import ExceptionCause
from instgen.helper import ISAInstrClass


def load_config(config_path: Path = Path(".") / "rvgen.toml") -> Dict[str, Any]:
    """Load the configuration from the TOML file."""

    if not config_path.exists():
        raise FileNotFoundError(
            f"Configuration file not found at {config_path}. "
            f"Please ensure the file exists or set the HARTATTACK_CONFIG environment variable."
        )

    try:
        with open(config_path, "r") as f:
            return toml.load(f)
    except toml.TomlDecodeError as e:
        raise ValueError(f"Invalid TOML configuration file: {e}")


def get_instruction_weights() -> Dict[ISAInstrClass, float]:
    """Get instruction class weights from configuration."""
    config = load_config()
    weights_config = config.get("instruction_weights", {})

    # Convert string keys to ISAInstrClass enum values
    weights = {}
    for key, value in weights_config.items():
        try:
            isa_class = ISAInstrClass[key]
            weights[isa_class] = float(value)
        except KeyError:
            raise ValueError(f"Unknown ISA instruction class: {key}")
        except (ValueError, TypeError):
            raise ValueError(f"Invalid weight value for {key}: {value}")

    weights[ISAInstrClass.CLEAR_MDT] = 0.0
    weights[ISAInstrClass.RESET_MCM] = 0.0

    return weights


def get_exception_weights() -> Dict[ExceptionCause, float]:
    """Get exception weights from configuration."""
    config = load_config()
    weights_config = config.get("exception_weights", {})

    # Convert string keys to ExceptionCause enum values
    weights = {}
    for key, value in weights_config.items():
        try:
            exception_cause = ExceptionCause[key]
            weights[exception_cause] = float(value)
        except KeyError:
            raise ValueError(f"Unknown exception cause: {key}")
        except (ValueError, TypeError):
            raise ValueError(f"Invalid weight value for {key}: {value}")

    return weights


def validate_config():
    """Validate the configuration file."""
    try:
        instruction_weights = get_instruction_weights()
        exception_weights = get_exception_weights()

        # Check that at least some instruction weights are non-zero
        if all(weight == 0 for weight in instruction_weights.values()):
            raise ValueError("All instruction weights cannot be zero")

        # Check that all weights are non-negative
        for isa_class, weight in instruction_weights.items():
            if weight < 0:
                raise ValueError(f"Negative weight for {isa_class.name}: {weight}")

        for exception_cause, weight in exception_weights.items():
            if weight < 0:
                raise ValueError(
                    f"Negative weight for {exception_cause.name}: {weight}"
                )

        return True
    except Exception as e:
        raise ValueError(f"Configuration validation failed: {e}")
