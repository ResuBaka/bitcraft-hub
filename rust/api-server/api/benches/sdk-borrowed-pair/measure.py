"""Balanced release CPU, per-process perf instructions, and separate allocation counts."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile

out, wire = (Path(p).resolve() for p in sys.argv[1:3])
core = sys.argv[3] if len(sys.argv) > 3 else "5"
private = tempfile.TemporaryDirectory(dir=out)
fallback = Path(private.name) / "fallback.wire"
subprocess.run([out / "target/release/fallback", wire, fallback], check=True, stdout=sys.stderr)
names = ("base", "candidate")
for case, path in [("all", wire), ("fallback", fallback)]:
    def command(name, mode):
        return ["taskset", "-c", core, out / name, path, mode]

    expected = None
    for name in names:
        check = subprocess.check_output(command(name, "check"), text=True).strip()
        if case == "all":
            assert check == "frames=104116 calls=245643 hash=bb0c984258351b7a", check
        if expected is not None:
            assert check == expected, (case, name, check, expected)
        expected = check
        data = subprocess.check_output(command(name + "-count", "cpu"), text=True)
        allocs = dict(item.split("=") for item in data.split())["alloc_calls_per_pass"]
        print(json.dumps(dict(case=case, name=name, check=check, allocations=int(allocs))), flush=True)
    for block in range(12):
        for name in names[::1 if block % 2 == 0 else -1]:
            p = subprocess.run(["perf", "stat", "-x;", "-e", "instructions:u,cycles:u", "--",
                                *command(name, "cpu")], capture_output=True, text=True, check=True)
            fields = dict(item.split("=") for item in p.stdout.split())
            counters = {row[2]: int(row[0]) for line in p.stderr.splitlines()
                        if len(row := line.split(";")) > 2 and row[2] in ("instructions:u", "cycles:u")}
            assert len(counters) == 2, p.stderr
            if case == "all":
                burst = subprocess.check_output(command(name, "burst"), text=True)
                fields.update(item.split("=") for item in burst.split())
                assert int(fields.pop("observed")) == 104116
            print(json.dumps(dict(case=case, block=block, name=name, **{k: float(v) for k, v in fields.items()},
                                  **counters)), flush=True)
