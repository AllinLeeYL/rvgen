#!/usr/bin/env python3
"""Count instructions executed by Spike before reaching an ELF's _exit symbol.

Examples:
    python3 scripts/count_spike_instructions.py elfs/test_00000.elf
    python3 scripts/count_spike_instructions.py program.elf --isa rv64gc
    python3 scripts/count_spike_instructions.py program.elf --start-pc 0x80000000
    python3 scripts/count_spike_instructions.py snippy.elf --symbol __snippy_exit
    python3 scripts/count_spike_instructions.py elfs/ --output stats.toml
    python3 scripts/count_spike_instructions.py elfs/ --recursive --timeout 30
    python3 scripts/count_spike_instructions.py elfs/ --output stats.toml --plot stats.png
    python3 scripts/count_spike_instructions.py --from-toml stats.toml --plot stats.svg

Counting requires Spike with commit logging and a RISC-V nm; no extra Python packages.
Optional plotting requires matplotlib (python3 -m pip install matplotlib).
Runs a bare-metal ELF on one hart (hart 0). Counts entries in --log-commits,
including boot/startup code and repeated loop iterations. --start-pc starts
counting at the first committed instruction at that PC, inclusively.
Spike stops BEFORE executing the target symbol's first instruction, even if
that instruction loops forever. Trapping instructions without a commit record
are not counted. Logs are consumed as a stream rather than saved to disk.
Pass a directory to run its *.elf files in sorted order; --recursive includes
subdirectories. Each ELF has its own timeout. Failures do not stop the batch.
Statistics (total, min, max, mean, median, population standard deviation) cover
successful runs only. --output saves per-file results and the summary as TOML
for .toml paths, or JSON otherwise. TOML omits unavailable fields (it has no null).
Each result includes a reason and readable reason_detail. summary.reason_statistics
stores instruction statistics per reason, including partial counts for failures.
--plot saves a distribution and statistics table grouped by reason, including
partial counts for failed runs. By default, successful runs use their last trap
cause (or termination reason if no trap); failures use their termination reason.
--plot-group-by termination or trap selects either field alone. Last trap causes
describe observed events, not proven reasons for termination. --from-toml (Python
3.11+) or --from-json plots a saved report without running Spike or requiring nm.
Trap diagnostics include Spike's cause, exception PC, instruction bits and tval.
Memory traps also report access_type (load, store/AMO, instruction_fetch) and
fault_address from Spike's tval, separately from the faulting instruction's PC.
This is the reported access address (virtual when translation is active), not
necessarily a physical address or the start of a split access. Missing tval is
reported as unavailable; non-memory traps do not interpret tval as an address.
The first --max-traps events and the last event are retained; all are counted.
All retained events are shown in the terminal and saved in the report.
Termination diagnostics distinguish reaching the stop symbol, a tohost exit,
timeout, interruption, and Spike errors. The instruction at the stop symbol is
inspected but not executed. Last-observed instructions are context, not proof of
what caused an external stop. Instruction-fetch faults/interrupts may have no bits.
Exit status: 0 on success, 1 on simulation failure, 2 on invalid input,
124 on a single-file timeout, and 130 on interruption. A completed batch
returns 1 if any ELF failed. Ctrl-C stops the batch and retains collected results.
"""

import argparse
from collections import Counter, deque
from dataclasses import dataclass
import json
import math
import os
from pathlib import Path
import re
import selectors
import shutil
import statistics
import subprocess
import sys
import tempfile
import time


COMMIT = re.compile(rb"^core\s+0:\s+[0-3]\s+0x([0-9a-fA-F]+)\s+\(0x([0-9a-fA-F]+)\)")
DISASSEMBLY = re.compile(rb"^core\s+0:\s+0x([0-9a-fA-F]+)\s+\(0x([0-9a-fA-F]+)\)\s*(.*)$")
TRAP = re.compile(rb"^core\s+0:\s+exception (.+?), epc 0x([0-9a-fA-F]+)\s*$")
TVAL = re.compile(rb"^core\s+0:\s+tval 0x([0-9a-fA-F]+)\s*$")
MEMORY_WRITE = re.compile(rb"\bmem\s+0x([0-9a-fA-F]+)\s+0x([0-9a-fA-F]+)")
PC = re.compile(rb"^0x([0-9a-fA-F]+)\s*$")
STOP_INSTRUCTION = re.compile(rb"^0x([0-9a-fA-F]+)\s+(\S.*)$")
LOG_NOISE = re.compile(rb"^core\s+0:\s+(?:>>>>|Executed \d+ times)")

MEMORY_TRAP_ACCESS = {
    f"trap_{operation}_{cause}": access
    for operation, access in (
        ("load", "load"), ("store", "store/AMO"), ("instruction", "instruction_fetch")
    )
    for cause in ("address_misaligned", "access_fault", "page_fault", "guest_page_fault")
}


def instruction(pc, bits, disassembly=None):
    return {"pc": hex(pc), "bits": f"0x{int(bits, 16):08x}", "disassembly": disassembly}


class TraceCapture:
    """Collect bounded diagnostic context without treating disassembly as commits."""

    def __init__(self, max_traps):
        self.max_traps = max_traps
        self.tohost_address = None
        self.tohost_exit_request = None
        self.reported_pcs = []
        self.stop_instruction = None
        self.last_instruction = None
        self.last_committed_instruction = None
        self.committed = 0
        self.trap_count = 0
        self.trap_causes = Counter()
        self.traps = []
        self.last_trap = None
        self.diagnostics = deque(maxlen=20)

    def consume(self, line):
        """Return a committed PC, or None for a diagnostic/disassembly line."""
        match = COMMIT.match(line)
        if match:
            pc = int(match[1], 16)
            current = instruction(pc, match[2])
            if (self.last_instruction and self.last_instruction["pc"] == current["pc"]
                    and self.last_instruction["bits"] == current["bits"]):
                current["disassembly"] = self.last_instruction["disassembly"]
            self.last_instruction = self.last_committed_instruction = current
            self.committed += 1
            if self.tohost_address is not None:
                for write in MEMORY_WRITE.finditer(line):
                    address, value = (int(field, 16) for field in write.groups())
                    # HTIF device 0 / command 0 / odd payload requests exit.
                    # Record the request even if the debugger stops before HTIF handles it.
                    if address == self.tohost_address:
                        self.tohost_exit_request = (
                            {"value": hex(value), "exit_code": value >> 1, "instruction": current}
                            if value & 1 and value >> 48 == 0 else None
                        )
            return pc
        match = DISASSEMBLY.match(line)
        if match:
            self.last_instruction = instruction(int(match[1], 16), match[2],
                                                match[3].decode("utf-8", errors="replace"))
            return None
        match = TRAP.fullmatch(line)
        if match:
            cause = match[1].decode("utf-8", errors="replace")
            epc = int(match[2], 16)
            # Fetch faults have no fetched instruction. Interrupts are not caused
            # by the previous instruction, even if the PC happens to match.
            fetched = not (cause.startswith("interrupt") or cause in {
                "trap_instruction_access_fault", "trap_instruction_page_fault",
                "trap_instruction_guest_page_fault",
            })
            faulting = (self.last_instruction if fetched and self.last_instruction
                        and self.last_instruction["pc"] == hex(epc) else None)
            self.last_trap = {
                "cause": cause, "epc": hex(epc), "tval": None,
                "access_type": MEMORY_TRAP_ACCESS.get(cause), "fault_address": None,
                "instruction": faulting, "committed_instructions_before": self.committed,
            }
            self.trap_count += 1
            self.trap_causes[cause] += 1
            if len(self.traps) < self.max_traps:
                self.traps.append(self.last_trap)
            return None
        match = TVAL.fullmatch(line)
        if match and self.last_trap is not None:
            self.last_trap["tval"] = hex(int(match[1], 16))
            if self.last_trap["access_type"] is not None:
                self.last_trap["fault_address"] = self.last_trap["tval"]
            return None
        match = PC.fullmatch(line)
        if match:
            self.reported_pcs.append(int(match[1], 16))
            return None
        match = STOP_INSTRUCTION.fullmatch(line)
        if match and len(self.reported_pcs) == 2:
            self.stop_instruction = instruction(self.reported_pcs[-1], match[1],
                                                match[2].decode("utf-8", errors="replace"))
            return None
        if line.strip() and not LOG_NOISE.match(line):
            message = line.decode("utf-8", errors="replace")
            self.diagnostics.append(message[:2000])
            print(message, file=sys.stderr)


def capture_termination_reason(result, trace, symbol):
    """Report observed stop evidence; a handled trap alone does not prove termination."""
    reason = result.reason
    request = trace.tohost_exit_request
    if (reason in {"spike_exit", "spike_error"} and request is not None
            and result.spike_exit_code == (request["exit_code"] & 0xff)):
        reason = "tohost_exit"
    detail = result.error or f"Reached {symbol} before executing its first instruction"
    if reason == "tohost_exit":
        detail = f"Guest requested exit code {request['exit_code']} through tohost before reaching {symbol}"
    return {
        "reason": reason,
        "detail": detail,
        "spike_exit_code": result.spike_exit_code,
        "stop_instruction": trace.stop_instruction,
        "last_instruction": trace.last_instruction,
        "last_committed_instruction": trace.last_committed_instruction,
        "tohost_exit_request": request,
    }


@dataclass
class CountResult:
    count: int
    exit_code: int = 0
    error: str | None = None
    reason: str = "stop_symbol_reached"
    spike_exit_code: int | None = None


def find_nm(requested):
    candidates = [requested] if requested else [
        "riscv64-unknown-elf-nm", "riscv64-linux-gnu-nm",
        "riscv32-unknown-elf-nm", "riscv32-linux-gnu-nm", "llvm-nm",
    ]
    for candidate in candidates:
        executable = shutil.which(candidate)
        if executable:
            return executable
    raise ValueError("nm executable not found; put a RISC-V nm on PATH or pass --nm PATH")


def read_symbols(elf, nm):
    result = subprocess.run(
        [nm, "--defined-only", str(elf)], capture_output=True,
        text=True, errors="replace", check=False,
    )
    if result.returncode:
        raise ValueError(f"nm failed: {result.stderr.strip() or result.stdout.strip()}")
    symbols = {}
    for line in result.stdout.splitlines():
        fields = line.split()
        if len(fields) == 3:
            symbols.setdefault(fields[2], set()).add(int(fields[0], 16))
    return symbols


def symbol_address(elf, symbol, symbols):
    addresses = symbols.get(symbol, set())
    if not addresses:
        raise ValueError(f"{elf}: symbol {symbol!r} not found; Please try --symbol __snippy_exit")
    if len(addresses) != 1:
        raise ValueError(f"{elf}: symbol {symbol!r} has multiple addresses")
    return next(iter(addresses))


def spike_lines(child, timeout):
    """Keep timeout effective even when Spike is silent or emits partial lines."""
    deadline = time.monotonic() + timeout
    pending = b""
    os.set_blocking(child.stderr.fileno(), False)
    with selectors.DefaultSelector() as selector:
        selector.register(child.stderr, selectors.EVENT_READ)
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise subprocess.TimeoutExpired(child.args, timeout)
            if not selector.select(remaining):
                continue
            chunk = os.read(child.stderr.fileno(), 65536)
            if not chunk:
                if pending:
                    yield pending
                child.wait(timeout=max(0, deadline - time.monotonic()))
                return
            lines = (pending + chunk).split(b"\n")
            pending = lines.pop()
            yield from lines


def count_instructions(args, elf, target, trace):
    count = 0
    total = 0
    started = args.start_pc is None
    outcome = None
    with tempfile.TemporaryDirectory(prefix="spike-count-") as directory:
        commands = Path(directory) / "commands.txt"
        # Check PC in the debugger, rather than waiting for _exit to commit:
        # this excludes the target instruction and also handles a trapping _exit.
        # Noisy stepping exposes faulting instructions, which have no commit record.
        commands.write_text(f"pc 0\nuntiln pc 0 {target:x}\npc 0\ninsn 0\nquit\n", encoding="ascii")
        command = [args.spike_path, "-p1", "-d", "--log-commits", f"--debug-cmd={commands}"]
        if args.isa:
            command.append(f"--isa={args.isa}")
        command.append(str(elf))
        with subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=sys.stderr,
                              stderr=subprocess.PIPE, bufsize=0) as child:
            try:
                for line in spike_lines(child, args.timeout):
                    pc = trace.consume(line)
                    if pc is not None:
                        total += 1
                        if args.start_pc is not None and pc == args.start_pc:
                            started = True
                        if started:
                            count += 1
            except subprocess.TimeoutExpired:
                outcome = CountResult(count, 124, f"Spike timed out after {args.timeout:g} seconds "
                                      f"before reaching {args.symbol}", "timeout")
            except KeyboardInterrupt:
                outcome = CountResult(count, 130, "interrupted", "interrupted")
            finally:
                if child.poll() is None:
                    child.kill()
                child.wait()

    if outcome is not None:
        outcome.spike_exit_code = child.returncode
        return outcome
    if child.returncode:
        return CountResult(count, 1, f"Spike exited with status {child.returncode}",
                           "spike_error", child.returncode)
    if len(trace.reported_pcs) != 2 or trace.reported_pcs[-1] != target:
        return CountResult(count, 1, f"Spike stopped without reaching {args.symbol} ({target:#x})",
                           "spike_exit", child.returncode)
    if not total and trace.reported_pcs[0] != target:
        return CountResult(count, 1, "no commit records received; Spike must support --log-commits",
                           "trace_error", child.returncode)
    # An empty interval is valid when the requested start is also the stop PC.
    if not started and args.start_pc != target:
        return CountResult(count, 1, f"start PC {args.start_pc:#x} was not found before {args.symbol}",
                           "start_pc_not_reached", child.returncode)
    return CountResult(count, spike_exit_code=child.returncode)


def run_elf(args, elf, nm):
    started = time.monotonic()
    target = None
    trace = TraceCapture(args.max_traps)
    try:
        symbols = read_symbols(elf, nm)
        target = symbol_address(elf, args.symbol, symbols)
        tohost = symbols.get("tohost", set())
        if len(tohost) == 1:
            trace.tohost_address = next(iter(tohost))
        result = count_instructions(args, elf, target, trace)
    except (OSError, ValueError) as error:
        result = CountResult(0, 2, str(error), "input_error")
    except KeyboardInterrupt:
        result = CountResult(0, 130, "interrupted", "interrupted")
    record = {
        "elf": str(elf),
        "status": {0: "ok", 124: "timeout", 130: "interrupted"}.get(result.exit_code, "error"),
        "reason": None,
        "reason_detail": None,
        "exit_code": result.exit_code,
        "instructions": result.count if result.exit_code == 0 else None,
        "partial_instructions": result.count if result.exit_code else None,
        "target_pc": hex(target) if target is not None else None,
        "elapsed_seconds": round(time.monotonic() - started, 6),
        "error": result.error,
        "termination": capture_termination_reason(result, trace, args.symbol),
        "trap_count": trace.trap_count,
        "trap_causes": dict(trace.trap_causes),
        "traps": trace.traps,
        "traps_omitted": trace.trap_count - len(trace.traps),
        "last_trap": trace.last_trap,
        "diagnostics": list(trace.diagnostics),
    }
    record["reason"] = result_reason(record)
    record["reason_detail"] = describe_result_reason(record)
    return record


def result_reason(result, group_by="reason"):
    """Use the same observed reason in saved statistics and plot groups."""
    trap = result.get("last_trap")
    if group_by == "trap":
        return trap["cause"] if trap else "no_trap"
    if group_by == "reason" and result["exit_code"] == 0 and trap:
        return trap["cause"]
    return result["termination"]["reason"]


def describe_result_reason(result):
    """Explain the selected reason without claiming a handled trap ended the run."""
    trap = result.get("last_trap")
    if result["exit_code"] != 0 or not trap:
        return result["termination"]["detail"]
    cause = trap["cause"].removeprefix("trap_").replace("_", " ")
    detail = f"Last observed trap: {cause}; instruction PC={trap['epc']}"
    access = trap.get("access_type", MEMORY_TRAP_ACCESS.get(trap["cause"]))
    if access is not None:
        address = trap.get("fault_address", trap["tval"])
        detail += f"; {access} address={address if address is not None else 'unavailable'}"
    if trap["instruction"]:
        text = trap["instruction"]["disassembly"] or trap["instruction"]["bits"]
        detail += "; instruction=" + " ".join(text.split())
    return detail


def instruction_statistics(counts):
    return {
        "total": sum(counts),
        "min": min(counts) if counts else None,
        "max": max(counts) if counts else None,
        "mean": statistics.mean(counts) if counts else None,
        "median": statistics.median(counts) if counts else None,
        "population_stddev": statistics.pstdev(counts) if counts else None,
    }


def summarize(results, selected):
    counts = [result["instructions"] for result in results if result["exit_code"] == 0]
    trap_causes = Counter()
    for result in results:
        trap_causes.update(result["trap_causes"])
    return {
        "selected": selected,
        "processed": len(results),
        "succeeded": len(counts),
        "failed": sum(result["exit_code"] not in (0, 130) for result in results),
        "timed_out": sum(result["exit_code"] == 124 for result in results),
        "interrupted": sum(result["exit_code"] == 130 for result in results),
        "not_run": selected - len(results),
        "runs_with_traps": sum(result["trap_count"] > 0 for result in results),
        "trap_count": sum(result["trap_count"] for result in results),
        "trap_causes": dict(trap_causes),
        "termination_reasons": dict(Counter(result["termination"]["reason"] for result in results)),
        "instructions": instruction_statistics(counts),
        "reason_statistics": {
            reason: {
                "runs": len(group["counts"]),
                "partial_runs": group["partial"],
                "instructions": instruction_statistics(group["counts"]),
            }
            for reason, group in group_instruction_counts(results).items()
        },
    }


def group_instruction_counts(results, group_by="reason"):
    """Group observed counts; failed runs contribute their partial counts."""
    if group_by not in {"reason", "termination", "trap"}:
        raise ValueError(f"unknown instruction grouping: {group_by}")
    if not isinstance(results, list):
        raise ValueError("report must contain a results list")
    groups = {}
    for index, result in enumerate(results):
        try:
            if type(result["exit_code"]) is not int:
                raise ValueError("exit_code must be an integer")
            partial = result["exit_code"] != 0
            count = result["partial_instructions" if partial else "instructions"]
            if type(count) is not int or count < 0:
                raise ValueError("instruction count must be a nonnegative integer")
            reason = result_reason(result, group_by)
            if not isinstance(reason, str) or not reason:
                raise ValueError("reason must be a nonempty string")
        except (KeyError, TypeError, ValueError) as error:
            raise ValueError(f"invalid result at index {index}: {error}") from error
        group = groups.setdefault(reason, {"counts": [], "partial": 0})
        group["counts"].append(count)
        group["partial"] += partial
    return dict(sorted(groups.items()))


def plotting_backend():
    """Load the optional dependency without selecting an interactive backend."""
    try:
        from matplotlib.backends.backend_agg import FigureCanvasAgg
        from matplotlib.figure import Figure
    except ImportError as error:
        raise ValueError("plotting requires matplotlib; install it with "
                         "python3 -m pip install matplotlib") from error
    return Figure, FigureCanvasAgg


def plot_instruction_statistics(results, path, group_by="reason"):
    """Save a stacked histogram and per-reason statistics as PNG, PDF, or SVG.

    Each run belongs to exactly one group. The default uses the last observed
    trap for successful runs, and termination reasons otherwise. Handled traps
    are context, not proof of what ended a run. Failed counts are partial and
    remain excluded from the existing successful-run JSON/terminal summary.
    """
    groups = group_instruction_counts(results, group_by)
    Figure, FigureCanvasAgg = plotting_backend()
    from matplotlib import colormaps
    from matplotlib.ticker import MaxNLocator, StrMethodFormatter
    from matplotlib.colors import to_rgba
    from textwrap import fill

    table_height = max(1.2, 0.36 * (len(groups) + 1))
    figure = Figure(figsize=(14, 5.5 + table_height), layout="constrained")
    figure.set_facecolor("#f8fafc")
    FigureCanvasAgg(figure)
    grid = figure.add_gridspec(2, 2, height_ratios=[4, table_height], width_ratios=[3, 1.4])
    histogram = figure.add_subplot(grid[0, 0])
    legend = figure.add_subplot(grid[0, 1])
    table_axes = figure.add_subplot(grid[1, :])
    legend.set_axis_off()
    table_axes.set_axis_off()
    grouping = {
        "reason": "Successful runs: last trap cause, or termination reason if no trap; "
                  "failed runs: termination reason",
        "termination": "Grouped by termination reason",
        "trap": "Grouped by last observed trap cause; no_trap means no trap was observed",
    }[group_by]
    figure.suptitle(f"Spike instruction counts  ·  {len(results):,} runs",
                   fontsize=20, fontweight="bold", color="#0f172a")
    histogram.set_title(fill(grouping, 100), loc="left", fontsize=9,
                        color="#64748b", pad=16)
    figure.supxlabel("Failed runs use partial counts, including zero for input errors. "
                     "Last trap causes are observations, not proven termination causes.", fontsize=9, color="#64748b")
    histogram.set_xlabel("Executed instructions per ELF (committed instructions)")
    histogram.set_ylabel("Number of ELFs")
    histogram.yaxis.set_major_locator(MaxNLocator(integer=True))
    histogram.yaxis.set_major_formatter(StrMethodFormatter("{x:,.0f}"))
    histogram.set_facecolor("#f8fafc")
    histogram.grid(axis="y", color="#e2e8f0", linewidth=0.7)
    histogram.spines[["top", "right", "left"]].set_visible(False)
    histogram.spines["bottom"].set_color("#cbd5e1")
    histogram.tick_params(axis="both", length=0, pad=8, labelcolor="#475569")
    histogram.xaxis.label.set_color("#475569")
    histogram.yaxis.label.set_color("#475569")
    histogram.xaxis.labelpad = 12
    histogram.yaxis.labelpad = 12
    histogram.set_axisbelow(True)
    if not groups:
        histogram.text(0.5, 0.5, "No processed runs to plot", ha="center", va="center",
                       transform=histogram.transAxes)
    else:
        counts = [group["counts"] for group in groups.values()]
        low, high = min(map(min, counts)), max(map(max, counts))
        # Integer-centered bins, capped at 40 to keep large count ranges legible.
        width = max(1, (high - low + 40) // 40)
        bins = [low - 0.5 + index * width
                for index in range((high - low) // width + 2)]
        palette = ["#2563eb", "#0d9488", "#d97706", "#7c3aed", "#e11d48", "#0891b2"]
        colors = [palette[index] if index < len(palette)
                  else colormaps["tab20"].colors[index % 20] for index in range(len(groups))]
        labels = [fill(reason.replace("_", " "), 32) + f"  (n={len(group['counts']):,})"
                  for reason, group in groups.items()]
        _, _, containers = histogram.hist(
            counts, bins=bins, stacked=True, label=labels, color=colors,
            rwidth=0.28, edgecolor="white", linewidth=0.4,
        )
        # Label occupied bin totals; per-group run counts appear in the legend/table.
        top_bars = containers if len(groups) == 1 else containers[-1]
        for bar in top_bars:
            total = bar.get_y() + bar.get_height()
            if total > 0:
                histogram.annotate(
                    f"{total:,.0f}", (bar.get_x() + bar.get_width() / 2, total),
                    xytext=(0, 5), textcoords="offset points", ha="center", va="bottom",
                    fontsize=8, fontweight="medium", color="#334155",
                )
        histogram.margins(x=0.03, y=0.16)
        histogram.xaxis.set_major_locator(MaxNLocator(integer=True))
        histogram.xaxis.set_major_formatter(StrMethodFormatter("{x:,.0f}"))
        handles, labels = histogram.get_legend_handles_labels()
        legend.legend(handles, labels, loc="center left", frameon=False, fontsize=9,
                      title="REASON · RUNS", title_fontsize=10, labelcolor="#334155",
                      labelspacing=1.3, handlelength=1, handleheight=1)
        rows = []
        for reason, group in groups.items():
            values = group["counts"]
            rows.append([
                reason, str(len(values)), str(group["partial"]), f"{sum(values):,}",
                f"{min(values):,}", f"{max(values):,}", f"{statistics.mean(values):,.2f}",
                f"{statistics.median(values):,.2f}", f"{statistics.pstdev(values):,.2f}",
            ])
        table = table_axes.table(
            cellText=rows,
            colLabels=["Reason", "Runs", "Partial", "Total", "Min", "Max", "Mean", "Median", "Pop. stddev"],
            colWidths=[0.31] + [0.69 / 8] * 8, cellLoc="center", bbox=[0, 0, 1, 1],
        )
        table.auto_set_font_size(False)
        table.set_fontsize(9)
        for (row, column), cell in table.get_celld().items():
            cell.set_edgecolor("#f8fafc")
            cell.set_linewidth(1.5)
            cell.set_text_props(color="#334155")
            if row == 0:
                cell.set_facecolor("#e2e8f0")
                cell.set_text_props(weight="bold", color="#0f172a")
            elif column == 0:
                cell.set_facecolor(to_rgba(colors[row - 1], 0.12))
                cell.set_text_props(ha="left")
            else:
                cell.set_facecolor("#ffffff" if row % 2 else "#f1f5f9")
    figure.savefig(path, dpi=180)


def print_scope(args):
    if args.start_pc is None:
        print("Includes Spike boot/startup code; excludes the instruction at the stop symbol.")
    else:
        print(f"Counted from {args.start_pc:#x} inclusive; excludes the instruction at the stop symbol.")


def print_summary(summary):
    print(f"\nELFs: {summary['succeeded']}/{summary['selected']} succeeded; "
          f"{summary['failed']} failed ({summary['timed_out']} timed out); "
          f"{summary['interrupted']} interrupted; {summary['not_run']} not run.")
    if summary["succeeded"]:
        counts = summary["instructions"]
        print(f"Instructions (successful runs): total={counts['total']}, "
              f"min={counts['min']}, max={counts['max']}, mean={counts['mean']:.2f}, "
              f"median={counts['median']:g}, population stddev={counts['population_stddev']:.2f}")
    else:
        print("No successful runs; instruction statistics are unavailable.")
    print("Termination reasons: " + ", ".join(
        f"{reason}={count}" for reason, count in sorted(summary["termination_reasons"].items())))
    if summary["trap_count"]:
        print(f"Traps: {summary['trap_count']} in {summary['runs_with_traps']} ELF(s); " + ", ".join(
            f"{cause}={count}" for cause, count in sorted(summary["trap_causes"].items())))


def describe_instruction(item):
    if item is None:
        return "instruction bits unavailable"
    return f"PC={item['pc']} bits={item['bits']}" + (
        f" ({item['disassembly']})" if item["disassembly"] else "")


def print_execution_details(result, indent="", stream=None):
    stream = sys.stdout if stream is None else stream
    termination = result["termination"]
    print(f"{indent}Termination: {termination['reason']} — {termination['detail']}", file=stream)
    if termination["stop_instruction"]:
        print(f"{indent}Stop instruction (not executed): "
              f"{describe_instruction(termination['stop_instruction'])}", file=stream)
    if termination["tohost_exit_request"]:
        request = termination["tohost_exit_request"]
        print(f"{indent}Guest exit request: code={request['exit_code']}, tohost={request['value']}; "
              f"{describe_instruction(request['instruction'])}", file=stream)
    elif termination["last_committed_instruction"]:
        print(f"{indent}Last committed instruction: "
              f"{describe_instruction(termination['last_committed_instruction'])}", file=stream)
    if result["trap_count"]:
        events = list(enumerate(result["traps"], 1))
        if result["trap_count"] > len(result["traps"]):
            events.append((result["trap_count"], result["last_trap"]))
        for index, trap in events:
            print(f"{indent}Trap {index}/{result['trap_count']}: {trap['cause']}, "
                  f"epc={trap['epc']}, tval={trap['tval']}; "
                  f"{describe_instruction(trap['instruction'])}", file=stream)
            access = trap.get("access_type", MEMORY_TRAP_ACCESS.get(trap["cause"]))
            if access is not None:
                fault_address = trap.get("fault_address", trap["tval"])
                location = (f"{fault_address} (Spike tval)" if fault_address is not None
                            else "unavailable (Spike did not report tval)")
                print(f"{indent}  Faulting memory access: {access} at {location}", file=stream)
        omitted = result["trap_count"] - len(events)
        if omitted:
            print(f"{indent}{omitted} intermediate trap event(s) omitted; "
                  "increase --max-traps to retain more.", file=stream)


def toml_report(report):
    """Encode this report's tables and arrays without an extra TOML dependency.

    None-valued fields are omitted because TOML has no null value. Empty tables
    and lists are retained, so empty batches and diagnostics still round-trip.
    """
    def quoted(value):
        return json.dumps(value, ensure_ascii=False).replace(chr(127), "\\u007f")

    def key(value):
        return value if re.fullmatch(r"[A-Za-z0-9_-]+", value) else quoted(value)

    def value(item):
        if isinstance(item, str):
            return quoted(item)
        if isinstance(item, bool):
            return str(item).lower()
        if isinstance(item, int) or isinstance(item, float) and math.isfinite(item):
            return repr(item)
        if isinstance(item, list):
            return "[" + ", ".join(value(entry) for entry in item) + "]"
        raise ValueError(f"unsupported TOML report value: {item!r}")

    def table_array(item):
        return isinstance(item, list) and bool(item) and all(isinstance(v, dict) for v in item)

    lines = [
        "# Spike instruction statistics",
        "# reason: last observed trap for completed counts; termination reason otherwise.",
        "# A handled trap is not proof of what terminated a run.",
        "# reason_statistics includes partial counts; summary.instructions excludes them.",
        "# Unavailable fields are omitted. Fault addresses come from Spike tval.",
        "",
    ]

    def table(items, path=(), array=False):
        if path:
            header = ".".join(key(part) for part in path)
            lines.extend(["", f"[[{header}]]" if array else f"[{header}]"])
        for name, item in items.items():
            if item is not None and not isinstance(item, dict) and not table_array(item):
                lines.append(f"{key(name)} = {value(item)}")
        for name, item in items.items():
            if isinstance(item, dict):
                table(item, (*path, name))
            elif table_array(item):
                for entry in item:
                    table(entry, (*path, name), array=True)

    table(report)
    return "\n".join(lines) + "\n"


def save_report(path, report):
    """Publish TOML or JSON atomically, including results from failed batches."""
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=path.parent,
                                         prefix=".spike-stats-", delete=False) as stream:
            temporary = Path(stream.name)
            if path.suffix.lower() == ".toml":
                stream.write(toml_report(report))
            else:
                json.dump(report, stream, indent=2)
                stream.write("\n")
        temporary.replace(path)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def run_inputs(args, elfs, nm, batch):
    results = []
    interrupted = False
    if batch:
        print(f"Running {len(elfs)} ELF(s); stop symbol: {args.symbol}; "
              f"timeout per ELF: {args.timeout:g}s.")
        print_scope(args)
    try:
        for index, elf in enumerate(elfs, 1):
            result = run_elf(args, elf, nm)
            results.append(result)
            if batch:
                label = elf.relative_to(args.elf)
                detail = (f"{result['instructions']} instructions" if result["exit_code"] == 0
                          else f"{result['status']}: {result['error']}")
                print(f"[{index}/{len(elfs)}] {label}: {detail}", flush=True)
            elif result["exit_code"] == 0:
                print(f"Reached {args.symbol} at {result['target_pc']} on hart 0.")
                print(f"Instructions before {args.symbol}: {result['instructions']}")
                print_scope(args)
            else:
                print(f"error: {result['error']}; "
                      f"{result['partial_instructions']} instructions counted so far", file=sys.stderr)
            print_execution_details(result, indent="  " if batch else "",
                                    stream=sys.stdout if batch or result["exit_code"] == 0 else sys.stderr)
            if result["exit_code"] == 130:
                interrupted = True
                break
    except KeyboardInterrupt:
        interrupted = True
    summary = summarize(results, len(elfs))
    if batch:
        print_summary(summary)
    if args.output:
        save_report(args.output, {
            "symbol": args.symbol,
            "start_pc": hex(args.start_pc) if args.start_pc is not None else None,
            "isa": args.isa,
            "timeout_seconds": args.timeout,
            "max_traps": args.max_traps,
            "interrupted": interrupted,
            "summary": summary,
            "results": results,
        })
        print(f"Saved statistics to {args.output}")
    if args.plot:
        plot_instruction_statistics(results, args.plot, args.plot_group_by)
        print(f"Saved figure to {args.plot}")
    if interrupted:
        return 130
    return int(summary["failed"] > 0) if batch else results[0]["exit_code"]


def positive_seconds(value):
    seconds = float(value)
    if not math.isfinite(seconds) or seconds <= 0:
        raise argparse.ArgumentTypeError("timeout must be a positive number of seconds")
    return seconds


def address(value):
    number = int(value, 0)
    if not 0 <= number < 1 << 64:
        raise argparse.ArgumentTypeError("PC must be an unsigned 64-bit address")
    return number


def nonnegative(value):
    number = int(value)
    if number < 0:
        raise argparse.ArgumentTypeError("must be nonnegative")
    return number


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("elf", type=Path, nargs="?",
                        help="bare-metal RISC-V ELF or directory of *.elf files")
    saved_input = parser.add_mutually_exclusive_group()
    saved_input.add_argument("--from-json", type=Path, metavar="FILE",
                             help="plot a saved JSON report without Spike (requires --plot)")
    saved_input.add_argument("--from-toml", type=Path, metavar="FILE",
                             help="plot a saved TOML report without Spike (requires --plot, Python 3.11+)")
    parser.add_argument("--plot", type=Path, metavar="FILE",
                        help="save instruction distribution and statistics as PNG, PDF, or SVG")
    parser.add_argument("--plot-group-by", choices=("reason", "termination", "trap"), default="reason",
                        help="plot grouping: reason uses last trap for successes and termination "
                             "for failures; termination or trap uses that field alone (default: reason)")
    parser.add_argument("--recursive", action="store_true", help="include subdirectories in folder mode")
    parser.add_argument("-o", "--output", type=Path, metavar="FILE",
                        help="save results and reasons as TOML (.toml) or JSON (.json)")
    parser.add_argument("--spike-path", default="spike", help="Spike executable (default: spike)")
    parser.add_argument("--nm", help="nm executable (default: discover a RISC-V nm on PATH)")
    parser.add_argument("--isa", help="ISA string passed to Spike (default: Spike's default)")
    parser.add_argument("--symbol", default="_exit", help="stop symbol (default: _exit)")
    parser.add_argument("--start-pc", type=address, help="count from this PC, e.g. 0x80000000")
    parser.add_argument("--timeout", type=positive_seconds, default=180, metavar="SECONDS",
                        help="simulation timeout per ELF in seconds (default: 180)")
    parser.add_argument("--max-traps", type=nonnegative, default=100, metavar="N",
                        help="retain the first N trap events per ELF, plus the last (default: 100)")
    args = parser.parse_args(argv)
    try:
        if args.plot:
            args.plot = args.plot.resolve()
            if args.plot.suffix.lower() not in {".png", ".pdf", ".svg"}:
                raise ValueError("--plot requires a .png, .pdf, or .svg file")
            if not args.plot.parent.is_dir() or args.plot.is_dir():
                Path(args.plot.parent).mkdir(parents=True, exist_ok=True)
            plotting_backend()
        saved_report = args.from_toml or args.from_json
        if saved_report:
            option = "--from-toml" if args.from_toml else "--from-json"
            if args.elf or args.output or args.recursive:
                raise ValueError(f"{option} cannot be combined with ELF input, --output, or --recursive")
            if not args.plot:
                raise ValueError(f"{option} requires --plot FILE")
            if args.plot == saved_report.resolve():
                raise ValueError("plot must not overwrite the input report")
            contents = saved_report.read_text(encoding="utf-8")
            if args.from_toml:
                try:
                    import tomllib
                except ImportError as error:
                    raise ValueError("--from-toml requires Python 3.11 or newer") from error
                report = tomllib.loads(contents)
            else:
                report = json.loads(contents)
            if not isinstance(report, dict) or not isinstance(report.get("results"), list):
                raise ValueError("report must contain a results list")
            plot_instruction_statistics(report["results"], args.plot, args.plot_group_by)
            print(f"Saved figure to {args.plot}")
            return 0
        if args.elf is None:
            raise ValueError("provide an ELF file/directory, --from-toml FILE, or --from-json FILE")
        if args.plot_group_by != "reason" and not args.plot:
            raise ValueError("--plot-group-by requires --plot FILE")
        args.elf = args.elf.resolve()
        batch = args.elf.is_dir()
        if batch:
            matches = args.elf.rglob("*.elf") if args.recursive else args.elf.glob("*.elf")
            elfs = sorted(path for path in matches if path.is_file())
            if not elfs:
                raise ValueError(f"no *.elf files found in {args.elf}")
        elif not args.elf.is_file():
            raise ValueError(f"ELF file not found: {args.elf}")
        else:
            if args.recursive:
                raise ValueError("--recursive requires a directory")
            elfs = [args.elf]
        if args.output:
            args.output = args.output.resolve()
            if args.output in {elf.resolve() for elf in elfs}:
                raise ValueError("output file must not overwrite an input ELF")
            if not args.output.parent.is_dir() or args.output.is_dir():
                Path(args.output.parent).mkdir(parents=True, exist_ok=True)
        if args.plot:
            if args.plot in {elf.resolve() for elf in elfs}:
                raise ValueError("plot must not overwrite an input ELF")
            if args.plot == args.output:
                raise ValueError("--plot and --output must name different files")
        nm = find_nm(args.nm)
        if not shutil.which(args.spike_path):
            raise ValueError(f"Spike executable not found: {args.spike_path}")
        return run_inputs(args, elfs, nm, batch)
    except (OSError, ValueError) as error:
        parser.exit(2, f"error: {error}\n")
    except KeyboardInterrupt:
        return 130


if __name__ == "__main__":
    sys.exit(main())
