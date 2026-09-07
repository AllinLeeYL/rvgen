"""Compare complete batch invocations; build the Rust release binary first."""

import argparse
import os
from pathlib import Path
import statistics
import subprocess
import sys
import tempfile
import time


def main():
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--count", type=int, default=1000)
    parser.add_argument("--size", type=int, default=256)
    parser.add_argument("--num-bbs", type=int, default=12)
    parser.add_argument("--seed", type=int, default=0)
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--rust-bin", type=Path, default=root / "rvgen/target/release/rvgen")
    parser.add_argument("--temp-dir", type=Path, help="Parent for temporary output directories; choose the filesystem to benchmark")
    args = parser.parse_args()
    if args.count < 1 or args.repeats < 1 or args.num_bbs < 1 or args.size < 0:
        parser.error("counts must be positive and size must be nonnegative")
    if not args.rust_bin.is_file():
        parser.error("build Rust first: cargo build --release --locked --manifest-path rvgen/Cargo.toml")
    env = dict(os.environ)
    env["PYTHONPATH"] = str(root / "src") + os.pathsep + env.get("PYTHONPATH", "")
    commands = {"Python": [sys.executable, "-m", "rvgen.cli"], "Rust": [str(args.rust_bin.resolve())]}
    common = ["--size", str(args.size), "--num-bbs", str(args.num_bbs), "--seed", str(args.seed)]
    timings = {name: [] for name in commands}
    print("Workload: class selection, runtime ELF construction, and file writes.", flush=True)
    print("Random instruction bodies remain unimplemented; compilation and simulation are excluded.", flush=True)
    try:
        with tempfile.TemporaryDirectory(prefix="rvgen-benchmark-", dir=args.temp_dir) as directory:
            directory = Path(directory)
            # Warm each executable once; alternate measurement order between repeats.
            runs = [(name, -1, 1) for name in commands]
            runs += [(name, repeat, args.count) for repeat in range(args.repeats)
                     for name in (list(commands) if repeat % 2 == 0 else list(reversed(commands)))]
            for name, repeat, count in runs:
                output = directory / f"{name}-{repeat}"
                started = time.perf_counter()
                result = subprocess.run(commands[name] + common + ["--count", str(count), "--out-dir", str(output)],
                                        cwd=root, env=env, capture_output=True, text=True)
                elapsed = time.perf_counter() - started
                if result.returncode:
                    raise RuntimeError(f"{name} failed: {result.stderr.strip()}")
                if len(list(output.glob("*.elf"))) != count:
                    raise RuntimeError(f"{name} did not produce {count} ELF files")
                if repeat >= 0:
                    timings[name].append(elapsed)
                    print(f"{name} run {repeat + 1}: {elapsed:.6f}s", flush=True)
    except (OSError, RuntimeError) as error:
        parser.exit(1, f"benchmark: {error}\n")
    medians = {name: statistics.median(values) for name, values in timings.items()}
    print("\nMedian wall time (includes process startup and writes; excludes cleanup):")
    for name, elapsed in medians.items():
        print(f"{name}: {elapsed:.6f}s, {args.count / elapsed:.2f} ELF/s")
    print(f"Python / Rust time: {medians['Python'] / medians['Rust']:.2f}x")


if __name__ == "__main__":
    main()
