"""Usage: python3 measure.py region14.wire cpu original_test_bin new_test_bin [--skills|--single]"""
import itertools
import json
import os
import re
import statistics
import subprocess
import sys

wire, cpu, original, candidate, *extra = sys.argv[1:]
if extra not in ([], ["--skills"], ["--single"]):
    raise SystemExit(__doc__)
label = {"--single": "handler", "--skills": "skills"}.get(next(iter(extra), ""), "board")
variants = [(name, binary, hints) for (name, binary), hints in itertools.product(
    [("original", original), ("candidate", candidate)], [True] if extra else [False, True]
)]
expected = None
if extra == ["--single"]:
    parity = []
    for binary in [original, candidate]:
        result = subprocess.check_output(
            ["taskset", "-c", cpu, binary, "replay_xp_parity", "--ignored", "--nocapture"],
            env=dict(os.environ, XP_WIRE=wire), text=True,
        )
        parity.append(re.search(r"scores=\w+ notifications=\d+ payloads=\w+", result).group())
    assert parity[0] == parity[1], parity
    print(parity[0], file=sys.stderr)
for block in range(8):
    offset = block % len(variants)
    order = variants[offset:] + variants[:offset]
    for name, binary, hints in (order if extra or block < 4 else reversed(order)):
        env = dict(os.environ, XP_WIRE=wire)
        env.pop("XP_NO_HINTS", None)
        if not hints:
            env["XP_NO_HINTS"] = "1"
        result = subprocess.check_output(
            ["taskset", "-c", cpu, binary, "replay_xp_worker", "--ignored", "--nocapture"],
            env=env, text=True,
        )
        rows = re.findall(r"worker_cpu_ns_per_update=([\d.]+) history=(\d+) digest=(\w+)", result)
        assert len(rows) == 7
        if expected is None:
            expected = rows[0][1:]
        assert all(row[1:] == expected for row in rows)
        print(json.dumps(dict(block=block, **{label: name}, hints=hints,
                              cpu_ns_per_update=statistics.median(float(r[0]) for r in rows),
                              history=int(expected[0]), digest=expected[1])), flush=True)
