#!/usr/bin/env python3
"""Fault-injection check for self-checking programs.

For each ELF, flip one random workload ALU instruction in `_stimulus` so it
computes a wrong value (add <-> sub, or the low bit of an OP-IMM immediate),
run the mutant on Spike, and classify how the program ends:

  pass        exit 0: the fault went unnoticed
  mismatch    exit 0xaa: caught by the final register check
  divergence  exit 0xab: caught by an entangled branch or jump (`_fail`)
  trap        any other exit code: caught by a trap (e.g. a wild address)

Mutants whose faulted instruction writes the same value as in the original
program (e.g. `ori` setting a bit that is already set) cannot be detected by
any check and are reported separately as `no effect`.

It also reports how many instructions ran between the fault and the end.

usage: inject_faults.py ELF_DIR [--isa rv64i_zicsr] [--mutants-per-elf 3]
"""
import argparse
import collections
import random
import re
import subprocess
import sys
from pathlib import Path

OUTCOMES = {0: "pass", 0xAA: "mismatch", 0xAB: "divergence"}


def symbols(elf):
    out = subprocess.run(
        ["riscv64-unknown-elf-nm", elf], capture_output=True, text=True, check=True
    ).stdout
    return {name: int(addr, 16) for addr, _, name in (l.split() for l in out.splitlines())}


def text_section(elf):
    """Return (address, bytes) of .text."""
    out = subprocess.run(
        ["riscv64-unknown-elf-objdump", "-h", elf], capture_output=True, text=True, check=True
    ).stdout
    m = re.search(r"\.text\s+([0-9a-f]+)\s+([0-9a-f]+)\s+[0-9a-f]+\s+([0-9a-f]+)", out)
    size, vma, offset = (int(x, 16) for x in m.groups())
    data = Path(elf).read_bytes()
    return vma, offset, data[offset : offset + size]


def instructions(vma, text, start, end):
    """Yield (addr, word) for each instruction in [start, end), C-aware."""
    pc = start
    while pc < end:
        lo = int.from_bytes(text[pc - vma : pc - vma + 2], "little")
        if lo & 3 == 3:
            yield pc, int.from_bytes(text[pc - vma : pc - vma + 4], "little")
            pc += 4
        else:
            pc += 2


def is_constant_chain(prev, word):
    """Whether `word` continues a generator constant load (lui/slli; addi rd, rd)
    rather than being a workload instruction."""
    if prev is None:
        return False
    rd, rs1 = (word >> 7) & 31, (word >> 15) & 31
    prev_opcode, prev_rd = prev & 0x7F, (prev >> 7) & 31
    prev_funct3 = (prev >> 12) & 7
    feeds = prev_opcode == 0x37 or (prev_opcode == 0x13 and prev_funct3 == 1) or prev_opcode == 0x1B
    return feeds and prev_rd == rd and rs1 == rd


def mutate(word, prev):
    """Return a mutated word computing a wrong value, or None."""
    opcode, rd, funct3 = word & 0x7F, (word >> 7) & 31, (word >> 12) & 7
    if rd == 0 or is_constant_chain(prev, word):
        return None
    if opcode == 0x33 and funct3 == 0 and (word >> 25) in (0, 0x20):
        return word ^ (1 << 30)  # add <-> sub
    if opcode == 0x13 and funct3 in (0, 4, 6):
        return word ^ (1 << 20)  # addi/xori/ori: flip imm bit 0
    return None


def run(spike, isa, elf, fault_pc, stops):
    """Run on Spike with a commit log; return (exit code, instrs between the
    fault and the first of `stops`, value the instruction at fault_pc wrote)."""
    proc = subprocess.run(
        [spike, f"--isa={isa}", "-l", "--log-commits", elf],
        capture_output=True, text=True, timeout=60,
    )
    after = written = None
    for line in proc.stderr.splitlines():
        # Commit lines: "core   0: 3 0x<pc> (0x<insn>) x<rd> 0x<value> ..."
        m = re.match(r"core\s+0: \d 0x([0-9a-f]+) \(0x[0-9a-f]+\)(?:\s+x\s*\d+\s+0x([0-9a-f]+))?", line)
        if not m:
            continue
        pc = int(m.group(1), 16) & 0xFFFFFFFF
        if after is not None:
            if pc in stops:
                break
            after += 1
        elif pc == fault_pc:
            after = 0
            written = m.group(2)
    return proc.returncode, after, written


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("elf_dir")
    ap.add_argument("--isa", default="rv64i_zicsr")
    ap.add_argument("--spike", default="spike")
    ap.add_argument("--mutants-per-elf", type=int, default=3)
    ap.add_argument("--seed", type=int, default=0)
    args = ap.parse_args()
    rng = random.Random(args.seed)

    outcomes = collections.Counter()
    latencies = collections.defaultdict(list)
    tmp = Path(args.elf_dir) / "mutant.elf.tmp"
    for elf in sorted(Path(args.elf_dir).glob("*.elf")):
        syms = symbols(elf)
        vma, offset, text = text_section(elf)
        candidates = []
        prev = None
        for pc, word in instructions(vma, text, syms["_stimulus"], syms["_check"]):
            if (m := mutate(word, prev)) is not None:
                candidates.append((pc, word, m))
            prev = word
        for pc, _, mutated in rng.sample(candidates, min(args.mutants_per_elf, len(candidates))):
            data = bytearray(elf.read_bytes())
            data[offset + pc - vma : offset + pc - vma + 4] = mutated.to_bytes(4, "little")
            tmp.write_bytes(data)
            stops = {syms[s] for s in ("_check", "_exit", "_fail", "_trap_handler") if s in syms}
            code, after, written = run(args.spike, args.isa, str(tmp), pc, stops)
            _, _, original = run(args.spike, args.isa, str(elf), pc, stops)
            outcome = OUTCOMES.get(code, "trap")
            if outcome == "pass" and written == original:
                outcome = "no effect"
            outcomes[outcome] += 1
            if after is not None:
                latencies[outcome].append(after)
    tmp.unlink(missing_ok=True)

    total = sum(outcomes.values())
    effective = total - outcomes["no effect"]
    caught = effective - outcomes["pass"]
    print(f"{total} mutants, {effective} effective, "
          f"{caught} caught ({100 * caught / max(effective, 1):.1f}% of effective)")
    for outcome in ("no effect", "pass", "mismatch", "divergence", "trap"):
        n = outcomes[outcome]
        lat = sorted(latencies[outcome])
        median = lat[len(lat) // 2] if lat else "-"
        print(f"  {outcome:<10} {n:4d} ({100 * n / max(total, 1):5.1f}%)  "
              f"median instrs from fault to end: {median}")


if __name__ == "__main__":
    sys.exit(main())
