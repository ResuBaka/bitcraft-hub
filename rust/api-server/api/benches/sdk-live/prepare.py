"""Build a private, instrumentable SDK checkout; never edit the API's Cargo pin."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess

here = Path(__file__).resolve().parent
api = here.parents[2]
p = argparse.ArgumentParser(description=__doc__)
p.add_argument("sdk", type=Path)
p.add_argument("out", type=Path)
p.add_argument("--revision", default="9e997ce63")
p.add_argument("--bindings-revision", help="archive this crafting-list revision's regional bindings")
args = p.parse_args()
out = args.out.resolve()
out.mkdir(exist_ok=False)
sdk = out / "sdk"
sdk.mkdir()
with subprocess.Popen(["git", "-C", args.sdk, "archive", args.revision], stdout=subprocess.PIPE) as archive:
    subprocess.run(["tar", "-x", "-C", sdk], stdin=archive.stdout, check=True)
    if archive.wait():
        raise RuntimeError("git archive failed")
# Keep git apply rooted in this scratch SDK even when out is inside another repo.
subprocess.run(["git", "init", "-q", sdk], check=True)
with (sdk / "sdks/rust/src/db_connection.rs").open("a") as f:
    f.write((here / "context.rs").read_text())
if args.bindings_revision:
    src = out / "game_module/src"
    src.mkdir(parents=True)
    tree = f"{args.bindings_revision}:rust/api-server/game_module/src"
    with subprocess.Popen(["git", "-C", api.parents[1], "archive", tree], stdout=subprocess.PIPE) as archive:
        subprocess.run(["tar", "-x", "-C", src], stdin=archive.stdout, check=True)
        if archive.wait():
            raise RuntimeError("bindings archive failed")
else:
    shutil.copytree(api / "game_module/src", out / "game_module/src")
dep = '{path="../sdk/sdks/rust",default-features=false,features=["streaming-decompression","tokio-websockets-rustls"]}'
(out / "game_module/Cargo.toml").write_text(
    '[package]\nname="game_module"\nversion="0.1.0"\nedition="2024"\n[dependencies]\nspacetimedb-sdk=' + dep)
driver = out / "driver"
driver.mkdir()
(driver / "Cargo.toml").write_text(
    '[workspace]\n[package]\nname="sdk-live"\nversion="0.0.0"\nedition="2024"\n'
    '[[bin]]\nname="sdk-live"\npath=' + json.dumps(str(here / "main.rs")) +
    '\n[features]\nalloc-count=[]\n[dependencies]\nlibc="0.2"\nmimalloc-safe={version="0.1.67",features=["v3"]}\ngame_module={path="../game_module"}\nspacetimedb-sdk=' + dep)
shutil.copy(api / "Cargo.lock", driver / "Cargo.lock")
