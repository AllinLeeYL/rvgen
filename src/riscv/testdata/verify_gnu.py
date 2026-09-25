#!/usr/bin/env python3
"""Verify the frozen fixtures against GNU RISC-V tools; never rewrite them."""
import argparse
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prefix", default="riscv64-unknown-elf-")
    args = parser.parse_args()
    groups = {32: [], 64: []}
    fixture = Path(__file__).with_name("gnu_encodings.tsv")
    for line in fixture.read_text().splitlines():
        if line.startswith("#"):
            continue
        name, xlen, word, operands, assembly = line.split("\t")
        groups[int(xlen)].append((name, bytes.fromhex(word)[::-1], assembly))

    with tempfile.TemporaryDirectory(prefix="rvgen-gnu-") as tmp:
        for xlen, cases in groups.items():
            base = Path(tmp) / f"rv{xlen}"
            source = [".text", ".option norelax", ".globl _start", "_start:"]
            for _, word, assembly in cases:
                source.extend([
                    ".option rvc" if len(word) == 2 else ".option norvc",
                    assembly,
                ])
            base.with_suffix(".s").write_text("\n".join(source) + "\n")
            commands = [
                [args.prefix + "as", f"-march=rv{xlen}imafdc_zicsr_zifencei_svinval",
                 "-mabi=" + ("ilp32d" if xlen == 32 else "lp64d"), "-mno-relax",
                 "-o", str(base.with_suffix(".o")), str(base.with_suffix(".s"))],
                [args.prefix + "ld", "-m", f"elf{xlen}lriscv", "--no-relax",
                 "-Ttext=0x100000", "-o", str(base.with_suffix(".elf")),
                 str(base.with_suffix(".o"))],
                [args.prefix + "objcopy", "-O", "binary", "--only-section=.text",
                 str(base.with_suffix(".elf")), str(base.with_suffix(".bin"))],
            ]
            for command in commands:
                subprocess.run(command, check=True, capture_output=True, text=True)
            actual = base.with_suffix(".bin").read_bytes()
            expected_size = sum(len(word) for _, word, _ in cases)
            if len(actual) != expected_size:
                raise RuntimeError(f"RV{xlen}: expected {expected_size} bytes, got {len(actual)}")
            offset = 0
            for _, word, assembly in cases:
                got = actual[offset:offset + len(word)]
                if got != word:
                    raise RuntimeError(f"RV{xlen} {assembly}: expected {word.hex()}, got {got.hex()}")
                offset += len(word)
    print(f"Verified {sum(map(len, groups.values()))} GNU assembler encodings.")


if __name__ == "__main__":
    main()
