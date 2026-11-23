"""Prepare a fresh private SDK and build release baseline/candidate from identical bindings."""
import os
from pathlib import Path
import shutil
import subprocess
import sys

here = Path(__file__).resolve().parent
sdk, out = map(lambda p: Path(p).resolve(), sys.argv[1:])
subprocess.run([sys.executable, here.parent / "sdk-live/prepare.py", sdk, out,
                "--revision", "842b520c2"], check=True)
driver = out / "driver"
bindings = out / "game_module/src/module_bindings/mod.rs"
assert "commit 842b520c2509b1cec478878a7a05e5f7bec6a906" in bindings.read_text()[:512]
conn = out / "sdk/sdks/rust/src/db_connection.rs"
direct = "self.apply_update(update, |_| Event::Transaction);"
text = conn.read_text()
start = "pub fn replay_tx(&self, update: M::DbUpdate) {\n        "
assert text.count(start + direct) == 1
conn.write_text(text.replace(start + direct,
    start + "self.process_message(ParsedMessage::TransactionUpdate(update)).unwrap();"))
with (driver / "Cargo.toml").open("a") as manifest:
    manifest.write('\n[profile.release]\nlto=true\ndebug="line-tables-only"\n')
    for name, path in [("fallback", here / "fallback.rs"),
                       ("lifecycle", here.parent / "sdk-owned-initial/lifecycle.rs")]:
        manifest.write(f'\n[[bin]]\nname="{name}"\npath="{path}"\n')

target = out / "target"
env = {**os.environ, "CARGO_TARGET_DIR": str(target)}
for name in ("base", "candidate"):
    if name == "candidate":
        subprocess.run(["git", "apply", here / "borrowed-pair.patch"], cwd=out / "sdk", check=True)
    for suffix, flags in [("", []), ("-count", ["--features", "alloc-count"])]:
        subprocess.run(["cargo", "build", "--bin", "sdk-live", "--release", "--offline", *flags],
                       cwd=driver, env=env, check=True)
        shutil.copy2(target / "release/sdk-live", out / (name + suffix))

for flags in ([], ["--features", "spacetimedb-sdk/client-cache"]):
    subprocess.run(["cargo", "build", "--offline", "--bin", "lifecycle", *flags],
                   cwd=driver, env={**env, "CARGO_PROFILE_DEV_DEBUG": "0"}, check=True)
    subprocess.run(["bun", here.parent / "sdk-owned-initial/lifecycle.js", target / "debug/lifecycle"],
                   check=True)
subprocess.run(["cargo", "build", "--bin", "fallback", "--release", "--offline"],
               cwd=driver, env=env, check=True)
