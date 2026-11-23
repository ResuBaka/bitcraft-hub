"""Build both release replays and validate the candidate with/without client caching.
Usage: python3 build.py SDK_REPO FRESH_OUTPUT_DIR REGENERATED_REGIONAL_MOD_RS
"""
from pathlib import Path
import os
import re
import shutil
import subprocess
import sys

here = Path(__file__).resolve().parent
sdk, out, generated = (Path(arg).resolve() for arg in sys.argv[1:])
subprocess.run([sys.executable, here.parent / "sdk-live/prepare.py", sdk, out,
                "--revision", "bcd163b7c", "--bindings-revision", "a5344722"], check=True)
conn = out / "sdk/sdks/rust/src/db_connection.rs"
text = conn.read_text()
start = "pub fn replay_tx(&self, update: M::DbUpdate) {\n        "
assert text.count(start + "self.apply_update(update, |_| Event::Transaction);") == 1
conn.write_text(text.replace(start + "self.apply_update(update, |_| Event::Transaction);",
                             start + "self.process_message(ParsedMessage::TransactionUpdate(update)).unwrap();"))
driver = out / "driver"
with (driver / "Cargo.toml").open("a") as manifest:
    manifest.write('\n[profile.release]\nlto=true\ndebug="line-tables-only"\n')
    for name, path in [("membership", here / "membership.rs"),
                       ("lifecycle", here.parent / "sdk-owned-initial/lifecycle.rs")]:
        manifest.write(f'\n[[bin]]\nname="{name}"\npath="{path}"\n')
target = out / "target"
env = {**os.environ, "CARGO_TARGET_DIR": str(target)}
for name in ("base", "candidate"):
    if name == "candidate":
        subprocess.run(["git", "apply", here / "registry.patch"], cwd=out / "sdk", check=True)
        bindings = out / "game_module/src/module_bindings/mod.rs"
        # Reject a stale generator or a different schema in this isolated A/B.
        impl = re.compile(r"impl<'r> __sdk::AppliedDiff<'r> for AppliedDiff<'r> \{.*?^}", re.M | re.S)
        def split_dispatch(code):
            code = re.sub(r"^// This was generated using spacetimedb cli version .*$", "", code, flags=re.M)
            block, = impl.findall(code)
            return impl.sub("", code), re.findall(r"__AppliedTableDiff::(\w+)\(diff\)", block), block
        old, new = map(split_dispatch, (bindings.read_text(), generated.read_text()))
        assert old[:2] == new[:2] and len(old[1]) == 329
        assert old[2].count("callbacks.has_callbacks(") == 329
        assert "has_callbacks(" not in new[2] and new[2].count("invoke_table_row_callbacks::<") == 329
        shutil.copy2(generated, bindings)
    subprocess.run(["cargo", "build", "--release", "--offline", "--bin", "sdk-live"],
                   cwd=driver, env=env, check=True)
    shutil.copy2(target / "release/sdk-live", out / name)
    for flags in ([], ["--features", "spacetimedb-sdk/client-cache"]):
        subprocess.run(["cargo", "build", "--offline", "--bin", "membership", "--bin", "lifecycle", *flags],
                       cwd=driver, env={**env, "CARGO_PROFILE_DEV_DEBUG": "0"}, check=True)
        subprocess.run([target / "debug/membership"], check=True)
        subprocess.run(["bun", here.parent / "sdk-owned-initial/lifecycle.js", target / "debug/lifecycle"], check=True)
