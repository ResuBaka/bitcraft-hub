"""python3 measure.py BASE_TEST_BIN NEW_TEST_BIN TEST CPU_LIST PAIRS"""
import json
import re
import subprocess
import sys

base, new, test, cpus, pairs = sys.argv[1:]
expected = {"concurrent"} if test == "bench_concurrent_worker_queue" else {"ready", "handoff", "batch1", "batch32"}
for block in range(int(pairs)):
    order = [("base", base), ("seal", new)]
    for variant, exe in (order if block % 2 == 0 else reversed(order)):
        result = {"block": block, "variant": variant}
        try:
            output = subprocess.check_output(
                ["taskset", "-c", cpus, exe, test, "--ignored", "--nocapture"],
                text=True, timeout=5,
            )
            ns = {k: float(v) for k, v in re.findall(r"(\w+) cpu_ns_per_message=([\d.]+)", output)}
            assert ns.keys() == expected, output
            result.update(ns)
        except subprocess.TimeoutExpired:
            result["timeout"] = True
        print(json.dumps(result), flush=True)
