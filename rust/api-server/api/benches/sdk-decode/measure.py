"""Run matched release processes on one core; output CPU ms per capture pass."""
import argparse
import json
import subprocess
import sys


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ("binary", "wire", "cpu"):
        p.add_argument(name)
    p.add_argument("--stateless", action="store_true", help="compare the simpler SDK patch with stock and reset+scratch")
    args = p.parse_args()
    prefix = ["taskset", "-c", args.cpu, args.binary, args.wire]
    subprocess.run(prefix + ["check", "all"], check=True, stdout=sys.stderr)
    modes = ["stock", "stateless", "small"] if args.stateless else ["stock", "slice", "reset", "small", "none"]
    for selection, phase in [("gzip-tx", "envelope"), ("snap", "typed"), ("all", "typed")]:
        for block in range(len(modes) * (2 if args.stateless else 1)):
            i = block % len(modes)
            order = modes[i:] + modes[:i]
            for mode in order if block < len(modes) else reversed(order):
                ms = float(subprocess.check_output(prefix + [mode, selection, phase]))
                print(json.dumps(dict(block=block, case=selection, phase=phase, mode=mode, cpu_ms=ms)), flush=True)


if __name__ == "__main__":
    main()
