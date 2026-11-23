"""Summarize completed pairs; reject missing samples."""
import json
from statistics import median
import sys

rows = [r for line in sys.stdin if "block" in (r := json.loads(line))]
assert len(rows) == 32
for metric in ["cpu_ms_per_1000", "burst_p99_ns", "service_p99_ns", "instructions:u", "cycles:u"]:
    values = {(r["block"], r["name"]): r[metric] for r in rows}
    base = [values[i, "base"] for i in range(16)]
    cand = [values[i, "candidate"] for i in range(16)]
    wins = sum(c < b for c, b in zip(cand, base))
    paired = median(100 * (c / b - 1) for c, b in zip(cand, base))
    print(f"{metric}: {median(base):g} -> {median(cand):g}; median paired {paired:+.2f}%; {wins}/16 lower")
