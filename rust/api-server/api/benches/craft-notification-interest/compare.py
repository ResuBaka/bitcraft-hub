"""Run pinned release test binaries in alternating order; write raw CPU samples."""
import argparse
import csv
import os
import subprocess
import sys
from collections import defaultdict
from statistics import median


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline")
    parser.add_argument("candidate")
    parser.add_argument("--cpu", type=int, default=8)
    parser.add_argument("--pairs", type=int, default=24)
    args = parser.parse_args()
    os.sched_setaffinity(0, {args.cpu})
    test = "crafting::notifications::replay::coalesced_flush_cpu"
    writer = csv.writer(sys.stdout, lineterminator="\n")
    writer.writerow(["pair", "arm", "matching", "rows", "flushes", "cpu_ns"])
    arms = [("baseline", args.baseline), ("candidate", args.candidate)]
    samples = defaultdict(list)
    for pair in range(args.pairs):
        for arm, binary in arms[::1 if pair % 2 == 0 else -1]:
            output = subprocess.check_output(
                [binary, "--exact", test, "--ignored", "--nocapture"], text=True
            )
            seen = set()
            for line in output.splitlines():
                # Single-thread libtest can prefix the first sample with the test name.
                if "sample," in line:
                    interest, rows, flushes, ns = line.split("sample,", 1)[1].split(",")
                    seen.add((interest, int(rows)))
                    writer.writerow([pair + 1, arm, interest, rows, flushes, ns])
                    samples[interest, int(rows), arm].append(int(ns) / int(flushes))
            assert seen == {(m, n) for m in ["false", "true"] for n in [0, 1, 2, 144, 472, 538]}
            sys.stdout.flush()
    print("matching rows baseline_ns/flush candidate_ns/flush candidate_faster_pairs", file=sys.stderr)
    for interest, n, arm in sorted(samples):
        if arm == "baseline":
            a, b = samples[interest, n, arm], samples[interest, n, "candidate"]
            print(interest, n, f"{median(a):.2f}", f"{median(b):.2f}",
                  sum(x > y for x, y in zip(a, b)), file=sys.stderr)


if __name__ == "__main__":
    main()
