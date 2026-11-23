"""Compare phase probes and the unchanged end-to-end CPU and burst cases."""
import json
import argparse
from pathlib import Path
import struct
import subprocess
from tempfile import TemporaryDirectory

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("root", type=Path)
parser.add_argument("wire", type=Path)
parser.add_argument("cpu")
parser.add_argument("--confirm", choices=["row", "route", "both", "placement"])
args = parser.parse_args()
root, wire, core = args.root.resolve(), args.wire.resolve(), args.cpu
names = ["base", args.confirm] if args.confirm else ["base", "row", "route", "both", "placement"]


def run(name, case, source=wire):
    return subprocess.check_output(["taskset", "-c", core, root/name, source, case], text=True)


def fields(output):
    return dict(field.split("=", 1) for field in output.split())


# The capture has no duplicate table entries in any one transaction. Exercise
# merging separately, including repeated keys in different query sets.
def u32(n):
    return struct.pack("<I", n)


def row(x):
    return struct.pack("<QQiiI", 17, 100, x, 3, 1)


def rowlist(data):
    return b"\x01" + u32(bool(data)) + (bytes(8) if data else b"") + u32(len(data)) + data


def table(name, pairs):
    return u32(len(name)) + name.encode() + u32(len(pairs)) + b"".join(
        b"\x00" + rowlist(ins) + rowlist(dele) for ins, dele in pairs)


def frame(sets):
    data = b"\x00\x04" + u32(len(sets)) + b"".join(
        u32(i) + u32(len(tables)) + b"".join(tables) for i, tables in enumerate(sets))
    return struct.pack("<BQ", 4, len(data)) + data


with TemporaryDirectory() as tmp:
    synth = Path(tmp)/"edge.wire"
    synth.write_bytes(frame([
        [table("stamina_state", [(struct.pack("<Qqf", 1, 0, 10.), b"")]),
         table("location_state", [(row(1), row(0)), (row(2), row(1))])],
        [table("location_state", [(row(3), row(2))])],
    ]))
    reference = fields(run("base", "check", synth))
    assert int(reference["calls"]) == 4, reference
    for name in names:
        assert fields(run(name, "check", synth)) == reference
    for tbl in [table("bad_table_name", []), table("location_state", [(b"\x01", b"")])]:
        synth.write_bytes(frame([[tbl]]))
        errors = []
        for name in names:
            proc = subprocess.run(["taskset", "-c", core, root/name, synth, "check"],
                                  capture_output=True, text=True)
            assert proc.returncode != 0 and "Err" in proc.stderr
            errors.append(proc.stderr.split("Err", 1)[1])
        assert len(set(errors)) == 1, errors

for name in names:
    assert fields(run(name,"check")) == {"frames":"104116","calls":"245643","hash":"bb0c984258351b7a"}
for block in range(8 if args.confirm else 2 * len(names)):
    i = block % len(names)
    order = names[i:] + names[:i]
    for name in order if block < len(names) else reversed(order):
        metrics = fields(("" if args.confirm else run(name, "phases")) + run(name, "cpu") + run(name, "burst"))
        assert int(metrics["observed"]) == 104116
        print(json.dumps(dict(block=block,name=name,**{k:float(v) for k,v in metrics.items()})),flush=True)
