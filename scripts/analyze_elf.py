#!/usr/bin/env python3
"""Disassemble a RISC-V ELF and report static instruction statistics (Python 3.10+).

Requires GNU objdump with RISC-V support; no third-party Python packages.

Examples:
    python3 scripts/analyze_elf.py elfs/test_00000.elf
    python3 scripts/analyze_elf.py program.elf --format json -o stats.json
    python3 scripts/analyze_elf.py program.elf --section .snippy.text.rx \
        --disassembly program.dis

Counts cover objdump's executable sections, including startup code and padding
decoded as instructions. They are not execution counts or a reachability analysis.
Aliases are disabled, so e.g. a 32-bit nop is counted as addi. C contains c.*
compressed mnemonics (including compressed FP); extension groups are exclusive,
while memory categories overlap them. Groups describe instruction families,
not all ISA extensions required to execute the program. Unrecognized families
are Other; data directives and undecodable encodings are excluded and reported.

Classification reference:
https://docs.riscv.org/reference/isa/unpriv/rv-32-64g.html
"""

import argparse
from collections import Counter
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys


I_INSTRUCTIONS = set("""
    lui auipc jal jalr beq bne blt bge bltu bgeu
    lb lh lw lbu lhu lwu ld sb sh sw sd
    addi slti sltiu xori ori andi slli srli srai
    add sub sll slt sltu xor srl sra or and
    addiw slliw srliw sraiw addw subw sllw srlw sraw
    fence ecall ebreak
""".split())
M_INSTRUCTIONS = set("""
    mul mulh mulhsu mulhu div divu rem remu mulw divw divuw remw remuw
""".split())
FP_OPERATIONS = "fadd fsub fmul fdiv fsqrt fsgnj fsgnjn fsgnjx fmin fmax feq flt fle fclass fmadd fmsub fnmsub fnmadd".split()
F_INSTRUCTIONS = {f"{op}.s" for op in FP_OPERATIONS} | {
    "flw", "fsw", "fmv.x.w", "fmv.w.x",
} | {f"fcvt.{i}.s" for i in ("w", "wu", "l", "lu")} | {
    f"fcvt.s.{i}" for i in ("w", "wu", "l", "lu")}
D_INSTRUCTIONS = {f"{op}.d" for op in FP_OPERATIONS} | {
    "fld", "fsd", "fmv.x.d", "fmv.d.x", "fcvt.s.d", "fcvt.d.s",
} | {f"fcvt.{i}.d" for i in ("w", "wu", "l", "lu")} | {
    f"fcvt.d.{i}" for i in ("w", "wu", "l", "lu")}
A_INSTRUCTIONS = {
    f"{op}.{width}"
    for op in "lr sc amoswap amoadd amoxor amoand amoor amomin amomax amominu amomaxu".split()
    for width in ("w", "d")
}
CSR_INSTRUCTIONS = set("csrrw csrrs csrrc csrrwi csrrsi csrrci".split())
PRIVILEGED_INSTRUCTIONS = set("""
    mret sret uret dret wfi sfence.vma hfence.vvma hfence.gvma
    sinval.vma sfence.w.inval sfence.inval.ir hinval.vvma hinval.gvma
""".split())
MEMORY_INSTRUCTIONS = {
    "integer_load": set("lb lh lw lbu lhu lwu ld c.lw c.ld c.lwsp c.ldsp c.lbu c.lhu c.lh".split()),
    "integer_store": set("sb sh sw sd c.sw c.sd c.swsp c.sdsp c.sb c.sh".split()),
    "fp_load": set("flh flw fld flq c.flw c.fld c.flwsp c.fldsp".split()),
    "fp_store": set("fsh fsw fsd fsq c.fsw c.fsd c.fswsp c.fsdsp".split()),
    "atomic": A_INSTRUCTIONS,
    "fence": set("""
        fence fence.i fence.tso sfence.vma hfence.vvma hfence.gvma
        sinval.vma sfence.w.inval sfence.inval.ir hinval.vvma hinval.gvma
    """.split()),
}
EXTENSION_ORDER = ("I", "C", "M", "F", "D", "A", "Zicsr", "Zifencei", "Privileged", "Other")
SECTION_RE = re.compile(r"^Disassembly of section (.+):$")
INSTRUCTION_RE = re.compile(
    r"^\s*[0-9a-fA-F]+:\s+"
    r"((?:[0-9a-fA-F]{2})+(?:[ \t]+(?:[0-9a-fA-F]{2})+)*)"
    r"[ \t]+(\S+)(?:\s|$)"
)


def base_mnemonic(mnemonic):
    """Ignore atomic ordering suffixes for classification, retaining them in counts."""
    return re.sub(r"\.(?:aqrl|aq|rl)$", "", mnemonic)


def extension_for(mnemonic):
    name = base_mnemonic(mnemonic)
    if name.startswith("c."):
        return "C"
    for extension, names in (
        ("I", I_INSTRUCTIONS), ("M", M_INSTRUCTIONS), ("F", F_INSTRUCTIONS),
        ("D", D_INSTRUCTIONS), ("A", A_INSTRUCTIONS), ("Zicsr", CSR_INSTRUCTIONS),
        ("Zifencei", {"fence.i"}), ("Privileged", PRIVILEGED_INSTRUCTIONS),
    ):
        if name in names:
            return extension
    return "Other"


def memory_kind(mnemonic):
    name = base_mnemonic(mnemonic)
    for kind, names in MEMORY_INSTRUCTIONS.items():
        if name in names:
            return kind
    return None


def read_elf_header(path):
    with path.open("rb") as stream:
        header = stream.read(64)
    if header[:4] != b"\x7fELF":
        raise ValueError(f"{path}: not an ELF file")
    if len(header) < 16 or header[4] not in (1, 2) or header[5] not in (1, 2):
        raise ValueError(f"{path}: invalid ELF identification")
    bits = 32 if header[4] == 1 else 64
    byteorder = "little" if header[5] == 1 else "big"
    if len(header) < (52 if bits == 32 else 64):
        raise ValueError(f"{path}: truncated ELF header")
    machine = int.from_bytes(header[18:20], byteorder)
    if machine != 243:  # EM_RISCV
        raise ValueError(f"{path}: expected a RISC-V ELF (e_machine={machine})")
    return {"bits": bits, "byteorder": byteorder}


def find_objdump(requested=None):
    if requested:
        executable = shutil.which(requested)
        if not executable:
            raise ValueError(f"objdump executable not found: {requested}")
        return executable
    names = ("riscv64-unknown-elf-objdump", "riscv64-linux-gnu-objdump",
             "riscv32-unknown-elf-objdump", "riscv32-linux-gnu-objdump")
    candidates = list(names)
    for variable in ("RISCV", "RISCV_TOOLCHAIN_PREFIX"):
        if os.environ.get(variable):
            candidates.extend(str(Path(os.environ[variable]) / "bin" / name) for name in names)
    for candidate in candidates:
        executable = shutil.which(candidate)
        if executable:
            return executable
    raise ValueError("RISC-V GNU objdump not found; put it on PATH or use --objdump /path/to/objdump")


def disassemble(path, objdump, sections):
    command = [objdump, "-d", "-z", "-w", "-M", "no-aliases"]
    for section in sections:
        command.extend(["-j", section])
    command.append(str(path.resolve()))
    result = subprocess.run(command, capture_output=True, text=True, errors="replace",
                            env={**os.environ, "LC_ALL": "C"}, check=False)
    if result.returncode:
        raise ValueError(f"objdump failed: {result.stderr.strip() or result.stdout.strip()}")
    # Do not silently accept a tool that ignored no-aliases or other options.
    if result.stderr.strip():
        raise ValueError(f"objdump reported: {result.stderr.strip()}")
    return result.stdout


def statistic(count, total):
    proportion = count / total if total else 0.0
    return {"count": count, "proportion": proportion, "percent": 100 * proportion}


def analyze(disassembly):
    counts, extensions, memory, sections = Counter(), Counter(), Counter(), Counter()
    excluded = Counter()
    instruction_bytes = excluded_bytes = 0
    section = None
    for line in disassembly.splitlines():
        match = SECTION_RE.match(line)
        if match:
            section = match[1]
            sections[section] += 0
            continue
        match = INSTRUCTION_RE.match(line)
        if not match or section is None:
            continue
        raw, mnemonic = match.groups()
        mnemonic = mnemonic.lower()
        size = len("".join(raw.split())) // 2
        if mnemonic.startswith(".") or mnemonic in {"unimp", "c.unimp", "(bad)", "<unknown>"}:
            excluded[mnemonic] += 1
            excluded_bytes += size
            continue
        counts[mnemonic] += 1
        extensions[extension_for(mnemonic)] += 1
        kind = memory_kind(mnemonic)
        if kind:
            memory[kind] += 1
        sections[section] += 1
        instruction_bytes += size

    total = sum(counts.values())
    memory_total = sum(memory.values())
    instructions = {}
    for name, count in sorted(counts.items(), key=lambda item: (-item[1], item[0])):
        extension = extension_for(name)
        kind = memory_kind(name)
        instructions[name] = {
            **statistic(count, total), "extension": extension, "memory_kind": kind,
            "percent_of_extension": 100 * count / extensions[extension],
            "percent_of_memory": 100 * count / memory_total if kind else None,
        }
    warnings = []
    if excluded:
        warnings.append("Data directives/undecodable encodings were excluded from instruction totals.")
    if extensions["Other"]:
        warnings.append("Other contains instruction families outside this classifier; memory statistics cover known scalar operations only.")
    if not total:
        warnings.append("No decoded instructions found in the selected executable sections.")
    return {
        "total_instructions": total,
        "unique_instructions": len(counts),
        "instruction_bytes": instruction_bytes,
        "extensions": {name: statistic(extensions[name], total) for name in EXTENSION_ORDER
                       if extensions[name] or name in ("I", "C", "M", "F")},
        "memory": {
            **statistic(memory_total, total),
            "categories": {kind: {**statistic(memory[kind], total),
                                  "percent_of_memory": 100 * memory[kind] / memory_total if memory_total else 0.0}
                           for kind in MEMORY_INSTRUCTIONS},
        },
        "sections": {name: statistic(count, total) for name, count in sections.items()},
        "instructions": instructions,
        "excluded": {"entries": sum(excluded.values()), "bytes": excluded_bytes,
                     "by_mnemonic": dict(excluded)},
        "warnings": warnings,
    }


def format_text(report):
    lines = [
        f"ELF: {report['file']} (RV{report['elf']['bits']}, {report['elf']['byteorder']}-endian)",
        f"Static instructions: {report['total_instructions']:,} | Unique mnemonics: {report['unique_instructions']} | Bytes: {report['instruction_bytes']:,}",
        "Proportions use all decoded instructions; aliases are disabled.",
        "Extension groups are exclusive: compressed FP is C; F/D count uncompressed FP.",
        "Memory groups overlap extensions and cover known scalar operations; atomic includes LR/SC.",
        "Counts include decoded padding/startup code, not runtime execution frequencies.",
    ]
    for title, values in (("Sections", report["sections"]), ("Extensions", report["extensions"]),
                          ("Memory categories", report["memory"]["categories"])):
        width = max([20] + [len(name) for name in values])
        lines.extend(["", title, f"{'Name':<{width}} {'Count':>10} {'% all':>9}"])
        for name, stats in values.items():
            lines.append(f"{name:<{width}} {stats['count']:>10,} {stats['percent']:>8.2f}%")
    memory = report["memory"]
    lines.append(f"Memory total: {memory['count']:,} ({memory['percent']:.2f}% of all instructions)")
    lines.extend(["", "Instructions (descending count)",
                  f"{'Mnemonic':<22} {'Group':<11} {'Memory':<14} {'Count':>10} {'% all':>9} {'% group':>9} {'% memory':>9}"])
    for name, stats in report["instructions"].items():
        share = "-" if stats["percent_of_memory"] is None else f"{stats['percent_of_memory']:.2f}%"
        lines.append(f"{name:<22} {stats['extension']:<11} {stats['memory_kind'] or '-':<14} "
                     f"{stats['count']:>10,} {stats['percent']:>8.2f}% {stats['percent_of_extension']:>8.2f}% {share:>9}")
    excluded = report["excluded"]
    lines.extend(["", f"Excluded data/undecodable entries: {excluded['entries']:,} ({excluded['bytes']:,} bytes)"])
    lines.extend(f"Warning: {warning}" for warning in report["warnings"])
    return "\n".join(lines) + "\n"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("elf", type=Path, help="RISC-V ELF executable or object file")
    parser.add_argument("--objdump", help="GNU objdump executable with RISC-V support")
    parser.add_argument("--section", action="append", default=[], help="analyze only this section (repeatable)")
    parser.add_argument("--format", choices=("text", "json"), default="text", help="report format (default: text)")
    parser.add_argument("-o", "--output", type=Path, help="save report to this file instead of stdout")
    parser.add_argument("--disassembly", type=Path, help="also save the canonical objdump disassembly")
    args = parser.parse_args(argv)
    try:
        # Prevent a report or disassembly path from overwriting the input or each other.
        paths = [path.resolve() for path in (args.elf, args.output, args.disassembly) if path is not None]
        if len(paths) != len(set(paths)):
            raise ValueError("ELF, report, and disassembly paths must be different")
        metadata = read_elf_header(args.elf)
        objdump = find_objdump(args.objdump)
        disassembly = disassemble(args.elf, objdump, args.section)
        report = analyze(disassembly)
        missing = set(args.section) - report["sections"].keys()
        if missing:
            raise ValueError(f"sections were not disassembled: {', '.join(sorted(missing))}")
        report.update({"file": str(args.elf), "elf": metadata, "objdump": objdump,
                       "counting_policy": "Static canonical instructions; exclusive extension families (compressed FP in C); overlapping known scalar memory operations; data/undecodable entries excluded."})
        output = json.dumps(report, indent=2) + "\n" if args.format == "json" else format_text(report)
        if args.disassembly:
            args.disassembly.write_text(disassembly, encoding="utf-8")
        if args.output:
            args.output.write_text(output, encoding="utf-8")
        else:
            sys.stdout.write(output)
    except (OSError, ValueError) as error:
        parser.exit(1, f"error: {error}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
