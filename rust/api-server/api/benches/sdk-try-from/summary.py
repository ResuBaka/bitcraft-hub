"""Median process results; count full-path wins against the same order block."""
import json
import statistics
import sys

rows = [json.loads(line) for line in sys.stdin if line.strip()]
control = {r["block"]: r for r in rows if r["name"] == "base"}
for name in ["base", "row", "route", "both", "placement"]:
    rs = [r for r in rows if r["name"] == name]
    if not rs:
        continue
    assert len(rs) == len(control)
    stats = {k: round(statistics.median(r[k] for r in rs), 3)
             for k in ["try_from_ns", "route_ns", "rows_ns", "append_ns",
                       "cpu_ms_per_1000", "burst_p99_ns", "service_p99_ns"] if k in rs[0]}
    print(name, stats, "paired wins", {k:sum(r[k] < control[r["block"]][k] for r in rs)
                                        for k in ["try_from_ns", "cpu_ms_per_1000", "burst_p99_ns"] if k in rs[0]})
