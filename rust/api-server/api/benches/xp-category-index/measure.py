"""Run the existing full XP handler replay on the private region-14 fixture."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import statistics
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("wire", type=Path)
parser.add_argument("control")
parser.add_argument("index")
args = parser.parse_args()
assert hashlib.sha256(args.wire.read_bytes()).hexdigest() == (
    "c5c7878915085400621d32396d72a8d93c37ca80e6e2fc13859a3613a3bb3997"
)
env = dict(os.environ, XP_WIRE=str(args.wire.resolve()))
env.pop("XP_NO_HINTS", None)
arms = [("control", args.control), ("index", args.index)]


def replay(binary, test):
    return subprocess.check_output(
        ["taskset", "-c", "8", binary, f"leaderboard::bitcraft::xp_replay::{test}",
         "--exact", "--ignored", "--nocapture"], env=env, text=True,
    )


for arm, binary in arms:
    line = re.search(r"scores=(\w+) notifications=(\d+) payloads=(\w+)",
                     replay(binary, "replay_xp_parity"))
    assert line and line.groups() == ("947a974b0863ea79", "113012", "6d5c24a6925b78e7"), line
    print(json.dumps(dict(arm=arm, scores=line[1], notifications=int(line[2]),
                          payloads=line[3])), flush=True)

for pair in range(8):
    for arm, binary in (arms if pair % 2 == 0 else reversed(arms)):
        rows = re.findall(r"worker_cpu_ns_per_update=([\d.]+) history=(\d+) digest=(\w+)",
                          replay(binary, "replay_xp_worker"))
        assert len(rows) == 7 and all(row[1:] == ("37649", "b7e623ea527650bd") for row in rows)
        passes = [float(row[0]) for row in rows]
        print(json.dumps(dict(pair=pair, arm=arm, cpu=8, ns_per_update=passes,
                              median=statistics.median(passes), history=37649,
                              digest=rows[0][2])), flush=True)
