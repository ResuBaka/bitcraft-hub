"""Check ordered callback contents; output warm CPU and burst-p99 samples."""
import json
from pathlib import Path
import subprocess
import sys

root, wire = (Path(p).resolve() for p in sys.argv[1:3])
cpu = sys.argv[3]


def run(name, case):
    text = subprocess.check_output(["taskset", "-c", cpu, root/name, wire, case], text=True)
    return dict(field.split("=", 1) for field in text.split())


variants = ["base", "inline", "context", "both"]
expected = run("base", "check")
assert int(expected["frames"]) == 104116 and int(expected["calls"]) > 0, expected
for name in variants:
    assert run(name, "check") == expected, name
    data = run(name+"-count", "cpu")
    print(json.dumps(dict(name=name, **expected, alloc_calls_per_pass=int(data["alloc_calls_per_pass"]))), flush=True)
for block in range(8):
    i = block % len(variants)
    order = variants[i:] + variants[:i]
    for name in order if block < len(variants) else reversed(order):
        result = {**run(name, "cpu"), **run(name, "burst")}
        assert int(result["observed"]) == int(expected["frames"]), result
        print(json.dumps(dict(block=block, name=name, **{k:float(v) for k,v in result.items()})), flush=True)
