import argparse
import random
import tomllib
from pathlib import Path

from rvgen.generator.generator import Generator, GeneratorParams


def parse_bool(val):
    return str(val).lower() in ("1", "true", "yes")


def config_keys(parser):
    """
    Return argument destinations that may be read from TOML.
    For example, if the parser has an argument --size and --memsize, this function will return "size" and "memsize".
    """
    return {
        action.dest
        for action in parser._actions
        if action.option_strings and action.dest != "config"
    }


def load_config(path, parser) -> dict:
    """
    Load config from TOML file and validate keys. 
    If path is None, return an empty dict.
    """
    if path is None:
        return {}

    with path.open("rb") as config_file:
        config = tomllib.load(config_file)

    unknown_keys = set(config) - config_keys(parser)
    if unknown_keys:
        raise ValueError("unknown config option(s): " + ", ".join(sorted(unknown_keys)))

    return config


def build_parser():
    parser = argparse.ArgumentParser(description="RISC-V instruction generator")

    parser.add_argument("--config", type=Path)

    parser.add_argument("--size", type=int, default=256)
    parser.add_argument("--memsize", type=int, default=4096)
    parser.add_argument("--num-cores", type=int, default=1)
    parser.add_argument("--num-bbs", type=int, default=12)
    parser.add_argument("--seed", type=int, default=0)
    parser.add_argument(
        "--authorize-privileges",
        type=parse_bool,
        default=True,
    )
    parser.add_argument("--out", "-o", type=Path, default=Path("output.elf"),
                        help="Output ELF file path")

    return parser


def parse_argument():
    parser = build_parser()

    # Read only --config first.
    config_args, _ = parser.parse_known_args()
    config = load_config(config_args.config, parser)

    # Config values become defaults; explicit CLI values still win.
    parser.set_defaults(**config)

    return parser.parse_args()


def main():
    args = parse_argument()

    generatorParams = GeneratorParams()
    generator = Generator(generatorParams=generatorParams)
    generator.generate()
    generator.gen_elf(args.out)


if __name__ == "__main__":
    main()
