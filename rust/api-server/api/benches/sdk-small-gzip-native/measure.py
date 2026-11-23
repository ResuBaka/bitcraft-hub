"""Paired full-process A/B, with a shared CPU lock; emit aggregate JSONL only."""
import fcntl
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

out, wire = map(Path, sys.argv[1:3])
cpu = sys.argv[3]
lock = open(os.environ.get("BENCH_CPU_LOCK", f"/tmp/opencode/cpu{cpu}.lock"), "a")
fcntl.flock(lock, fcntl.LOCK_EX)

def sample(arm, mode, subset):
    events = {}
    with tempfile.TemporaryFile("w+") as stdout, tempfile.TemporaryFile("w+") as perf:
        cmd = ["taskset", "-c", cpu]
        if mode == "cpu":
            cmd += ["perf", "stat", "-x", ";", "--no-big-num", "-o", f"/proc/self/fd/{perf.fileno()}",
                    "-e", "instructions:u,cycles:u", "--"]
        cmd += [str(out / arm), str(wire), mode, subset]
        with subprocess.Popen(cmd, stdout=stdout, pass_fds=[perf.fileno()]) as proc:
            _, status, usage = os.wait4(proc.pid, 0)
            proc.returncode = os.waitstatus_to_exitcode(status)
            if proc.returncode:
                raise subprocess.CalledProcessError(proc.returncode, cmd)
            rss = usage.ru_maxrss
        stdout.seek(0)
        data = stdout.read()
        perf.seek(0)
        for line in perf:
            fields = line.split(";")
            if len(fields) > 3 and fields[2] in ("instructions:u", "cycles:u"):
                events[fields[2]] = int(fields[0])
        assert mode != "cpu" or len(events) == 2
    parts = dict(field.split("=") for field in data.split())
    return {**{k: (v if k == "hash" else float(v) if "." in v else int(v))
               for k, v in parts.items()}, "peak_rss_kib": rss, **events}

def emit(**fields):
    print(json.dumps(fields), flush=True)

for subset in ("mixed", "gzip-tx"):
    parity = []
    for arm in ("base", "candidate"):
        s = sample(arm, "check", subset)
        parity.append((s["frames"], s["calls"], s["hash"]))
        emit(subset=subset, mode="check", arm=arm, **s)
    assert parity[0] == parity[1], "ordered transcript mismatch"
for pair in range(int(sys.argv[4]) if len(sys.argv) > 4 else 10):
    for subset in ("mixed", "gzip-tx"):
        for mode in ("cpu", "burst"):
            for arm in (("base", "candidate") if pair % 2 == 0 else ("candidate", "base")):
                emit(pair=pair, subset=subset, mode=mode, arm=arm, **sample(arm, mode, subset))
