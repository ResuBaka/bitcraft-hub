import json
import statistics
import sys

rows = [json.loads(line) for line in sys.stdin]
for case in ("all", "fallback"):
    samples = {(r["block"], r["name"]): r for r in rows if r["case"] == case and "block" in r}
    for key in ("cpu_ms_per_1000", "instructions:u", "cycles:u", "burst_p99_ns", "service_p99_ns"):
        if key not in samples[0, "base"]:
            continue
        base, candidate = ([samples[i, n][key] for i in range(len(samples) // 2)]
                           for n in ("base", "candidate"))
        changes = [100 * (b / a - 1) for a, b in zip(base, candidate)]
        print(f"{case:8} {key:16} {statistics.median(base):.5f} -> {statistics.median(candidate):.5f}"
              f"  paired {statistics.median(changes):+.2f}%  lower {sum(c < 0 for c in changes)}/{len(changes)}")
