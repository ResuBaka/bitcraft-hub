"""Summarize measure.py JSONL; each input block must contain all four variants."""
import json
import statistics
import sys


def main():
    names = ("base", "inline", "context", "both")
    rows = [json.loads(line) for line in sys.stdin if line.strip()]
    allocations = {r["name"]: r["alloc_calls_per_pass"] for r in rows if "block" not in r}
    timing = {(r["block"], r["name"]): r for r in rows if "block" in r}
    print("variant   alloc/pass   ms CPU/1k   burst p99 µs   service p99 µs   blocks below baseline (CPU/burst)")
    for name in names:
        def times(m):
            return [timing[i, name][m] for i in range(8)]

        def wins(m):
            return sum(v < timing[i, "base"][m] for i, v in enumerate(times(m)))

        cpu = statistics.median(times("cpu_ms_per_1000"))
        burst = statistics.median(times("burst_p99_ns")) / 1e3
        service = statistics.median(times("service_p99_ns")) / 1e3
        print(f"{name:8s} {allocations[name]:10,d} {cpu:11.4f} {burst:14.2f} {service:16.2f}"
              f"   {wins('cpu_ms_per_1000')}/8, {wins('burst_p99_ns')}/8")


if __name__ == "__main__":
    main()
