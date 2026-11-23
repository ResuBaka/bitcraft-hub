import collections
import json
from statistics import median
import sys

samples = collections.defaultdict(dict)
for line in sys.stdin:
    row = json.loads(line)
    if row["mode"] == "check":
        continue
    for field in ("cpu_ms_per_1000", "burst_p99_ns", "service_p99_ns", "instructions:u", "cycles:u", "peak_rss_kib"):
        if field in row:
            samples[row["subset"], row["mode"], field][row["pair"], row["arm"]] = row[field]
for key, values in samples.items():
    pairs = [(values[p, "base"], values[p, "candidate"]) for p in sorted({p for p, _ in values})]
    a, b = zip(*pairs)
    d = [(y / x - 1) * 100 for x, y in pairs]
    print(*key, f"{median(a):.5f} -> {median(b):.5f} ({median(d):+.2f}%; {sum(y < x for x,y in pairs)}/{len(pairs)} lower)")
