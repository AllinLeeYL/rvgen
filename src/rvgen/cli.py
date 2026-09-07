import argparse
import sys
import time
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
        if action.option_strings and action.dest not in {"config", "help"}
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
    parser.add_argument("--count", type=int, default=1,
                        help="Number of ELFs to generate in this process (default: 1)")
    parser.add_argument("--out-dir", type=Path,
                        help="Output directory, created if needed; batch files use OUT's numbered basename")

    return parser


def parse_argument(argv=None):
    parser = build_parser()

    # Read only --config first.
    config_args, _ = parser.parse_known_args(argv)
    try:
        config = load_config(config_args.config, parser)
        for key, value in config.items():
            if key in {"out", "out_dir"}:
                if not isinstance(value, str):
                    raise ValueError(f"{key} must be a string")
                config[key] = Path(value)
            elif key == "authorize_privileges":
                if type(value) is not bool:
                    raise ValueError(f"{key} must be a boolean")
            elif type(value) is not int:
                raise ValueError(f"{key} must be an integer")
    except (OSError, ValueError) as error:
        parser.error(str(error))

    # Config values become defaults; explicit CLI values still win.
    parser.set_defaults(**config)

    args = parser.parse_args(argv)
    if args.count <= 0 or args.num_cores <= 0 or args.num_bbs <= 0:
        parser.error("count, num_cores, and num_bbs must be greater than zero")
    if args.size < 0 or args.memsize < 0:
        parser.error("size and memsize must be nonnegative")
    if not -(1 << 63) <= args.seed <= args.seed + args.count - 1 < (1 << 63) or args.count > (1 << 63):
        parser.error("batch seed range exceeds signed 64-bit integers")
    return args


def output_path(args, index):
    directory = args.out_dir if args.out_dir is not None else args.out.parent
    if args.count == 1:
        return directory / args.out.name
    return directory / f"{args.out.stem}_{index:06}{args.out.suffix or '.elf'}"


def main(argv=None):
    args = parse_argument(argv)
    try:
        if args.num_cores != 1:
            raise ValueError("only one core is supported for ELF output")
        started = time.perf_counter()
        if args.out_dir is not None or args.count > 1:
            output_path(args, 0).parent.mkdir(parents=True, exist_ok=True)
        for index in range(args.count):
            params = GeneratorParams(size=args.size, memsize=args.memsize, num_cores=args.num_cores,
                                     num_bbs=args.num_bbs, seed=args.seed + index,
                                     authorize_privileges=args.authorize_privileges)
            generator = Generator(generatorParams=params)
            generator.generate()
            generator.gen_elf(output_path(args, index))
        elapsed = time.perf_counter() - started
        print(f"Generated {args.count} ELF(s) in {elapsed:.6f}s ({args.count / max(elapsed, 1e-300):.2f} ELF/s)",
              file=sys.stderr)
    except (OSError, ValueError, OverflowError) as error:
        print(f"rvgen: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
