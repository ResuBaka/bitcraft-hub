"""Summarize the captured batch mix and paired replay output."""
import json
import math
from pathlib import Path
from statistics import median
import sys


def pct(xs, fraction):
    return sorted(xs)[math.ceil(fraction * len(xs)) - 1]


lines = Path(sys.argv[1]).read_text().splitlines()
mix, = (json.loads(line[4:]) for line in lines if line.startswith("mix "))
sizes = mix["sizes"]
print(f'{mix["seed"]} seed rows; {mix["inserts"]} inserts, {mix["conflicts"]} conflicts')
print(f"{len(sizes)} batches, {sum(sizes)} rows; "
      f"min/p50/p95/max={min(sizes)}/{pct(sizes, .5)}/{pct(sizes, .95)}/{max(sizes)}; "
      f"{len(set(sizes))} VALUES shapes")
samples = [json.loads(line[7:]) for line in lines if line.startswith("sample ")]
arms = {w: {s["block"]: s for s in samples if s["writer"] == w}
        for w in ("values", "unnest")}
blocks = sorted(arms["values"])
assert blocks and blocks == sorted(arms["unnest"])
assert all(len(s["ns"]) == len(sizes) for s in samples)
print("metric | VALUES | UNNEST | UNNEST lower (paired blocks)")
metrics = [
    ("total write ms", lambda s: sum(s["ns"]) / 1e6),
    ("client CPU ms", lambda s: s["cpu_ns"] / 1e6),
    ("p50 flush µs", lambda s: pct(s["ns"], .50) / 1e3),
    ("p95 flush µs", lambda s: pct(s["ns"], .95) / 1e3),
]
for lo, hi in ((1, 1), (2, 4), (5, 16)):
    indices = [i for i, n in enumerate(sizes) if lo <= n <= hi]
    if indices:
        metrics.append((f"p50 {lo}–{hi} rows µs (n={len(indices)})",
                        lambda s, ix=indices: median(s["ns"][i] for i in ix) / 1e3))
for name, metric in metrics:
    v, u = ([metric(arm[b]) for b in blocks] for arm in arms.values())
    print(f"{name} | {median(v):.2f} | {median(u):.2f} | "
          f"{sum(y < x for x, y in zip(v, u))}/{len(blocks)}")
