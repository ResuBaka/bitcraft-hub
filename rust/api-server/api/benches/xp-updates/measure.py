"""Usage: python3 measure.py region14.wire cpu original_board_test_bin new_board_test_bin"""
import itertools
import json
import os
import re
import statistics
import subprocess
import sys

wire, cpu, original, candidate = sys.argv[1:]
variants = [(name, binary, hints) for (name, binary), hints in itertools.product(
    [("original", original), ("candidate", candidate)], [False, True]
)]
expected = None
for block in range(8):
    order = variants[block % 4:] + variants[:block % 4]
    for name, binary, hints in (order if block < 4 else reversed(order)):
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
        print(json.dumps(dict(block=block, board=name, hints=hints,
                              cpu_ns_per_update=statistics.median(float(r[0]) for r in rows),
                              history=int(expected[0]), digest=expected[1])), flush=True)
