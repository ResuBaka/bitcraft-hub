"""Run balanced paired processes; perf counters include process startup and the warm pass."""
import json
from pathlib import Path
import subprocess
import sys

out, wire = map(Path, sys.argv[1:3])
core = sys.argv[3]
names = ("base", "candidate")
for name in names:
    check = subprocess.check_output([out / name, wire, "check"], text=True).strip()
    assert check == "frames=104116 calls=245643 hash=bb0c984258351b7a", (name, check)
    print(json.dumps(dict(name=name, check=check)), flush=True)

for block in range(16):
    for name in names[::1 if block % 2 == 0 else -1]:
        cmd = ["taskset", "-c", core, out / name, wire]
        p = subprocess.run(["perf", "stat", "-x;", "-e", "instructions:u,cycles:u", "--",
                            *cmd, "cpu"], capture_output=True, text=True, check=True)
        burst = subprocess.check_output([*cmd, "burst"], text=True)
        fields = dict(item.split("=") for item in (p.stdout + burst).split())
        assert int(fields["observed"]) == 104116
        counters = {row[2]: int(row[0]) for line in p.stderr.splitlines()
                    if len(row := line.split(";")) > 2 and row[2] in ("instructions:u", "cycles:u")}
        assert len(counters) == 2, p.stderr
        print(json.dumps(dict(block=block, name=name, **{k: float(v) for k, v in fields.items()},
                              **counters)), flush=True)
