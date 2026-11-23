"""Build all four variants inside a fresh prepare.py directory."""
import os
from pathlib import Path
import shutil
import subprocess
import sys

root = Path(sys.argv[1]).resolve()
here = Path(__file__).resolve().parent
target = Path(os.environ.get("CARGO_TARGET_DIR", root / "target")).resolve()
env = {**os.environ, "CARGO_TARGET_DIR": str(target)}


def patch(name, reverse=False):
    subprocess.run(["git", "apply", *(["-R"] if reverse else []), here / f"{name}.patch"],
                   cwd=root / "sdk", check=True)


def build(name):
    for suffix, features in [("", []), ("-count", ["--features", "alloc-count"])]:
        subprocess.run(["cargo", "build", "--release", "--offline", *features],
                       cwd=root / "driver", env=env, check=True)
        shutil.copy2(target / "release/sdk-live", root / (name + suffix))


build("base")
patch("inline-events")
build("inline")
patch("shared-context")
build("both")
patch("inline-events", reverse=True)
build("context")
patch("shared-context", reverse=True)
