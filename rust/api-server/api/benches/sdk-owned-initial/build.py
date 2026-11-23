"""Build the pinned baseline and measured SDK patch in a private copy."""
import os
from pathlib import Path
import shutil
import subprocess
import sys

here = Path(__file__).resolve().parent
live = here.parent / "sdk-live"
sdk, out = (Path(arg).resolve() for arg in sys.argv[1:])
subprocess.run([sys.executable, live / "prepare.py", sdk, out, "--revision", "6beb46da7",
                "--bindings-revision", "1184a055"], check=True)
conn = "sdks/rust/src/db_connection.rs"
fixture = out / "sdk" / conn
original = fixture.read_text()
# Exercise the same ParsedMessage boundary on both builds.
hook = "pub fn replay_tx(&self, update: M::DbUpdate) {\n        "
old = hook + "self.apply_update(update, |_| Event::Transaction);"
assert original.count(old) == 1
fixture.write_text(original.replace(old, hook + "self.process_message(ParsedMessage::TransactionUpdate(update)).unwrap();"))
manifest = out / "driver/Cargo.toml"
with manifest.open("a") as f:
    f.write('\n[profile.release]\nlto=true\ndebug="line-tables-only"\n')
    f.write(f'\n[[bin]]\nname="lifecycle"\npath="{here / "lifecycle.rs"}"\n')
target = out / "target"
for name in ("base", "candidate"):
    if name == "candidate":
        subprocess.run(["git", "apply", here / "guard.patch"], cwd=out / "sdk", check=True)
    env = {**os.environ, "CARGO_TARGET_DIR": str(target)}
    subprocess.run(["cargo", "build", "--offline", "--release", "--bin", "sdk-live"],
                   cwd=out / "driver", env=env, check=True)
    shutil.copy2(target / "release/sdk-live", out / name)
    for features in ([], ["--features", "spacetimedb-sdk/client-cache"]):
        subprocess.run(["cargo", "build", "--offline", "--bin", "lifecycle", *features], cwd=out / "driver",
                       env={**env, "CARGO_PROFILE_DEV_DEBUG": "0"}, check=True)
        subprocess.run(["bun", here / "lifecycle.js", target / "debug/lifecycle"], check=True)
