"""Build matched A/B executable copies under a fresh private directory."""
import os
import argparse
from pathlib import Path
import re
import shutil
import subprocess
import sys

here = Path(__file__).resolve().parent
live = here.parent / "sdk-live"
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("sdk", type=Path)
parser.add_argument("out", type=Path)
parser.add_argument("--confirm", choices=["row", "route", "both", "placement"], help="build just base and candidate, with release LTO and no diagnostic methods")
args = parser.parse_args()
sdk, root = args.sdk.resolve(), args.out.resolve()
subprocess.run([sys.executable, live/"prepare.py", sdk, root, "--revision", "7303225f0",
                "--bindings-revision", "5a4d91b"], check=True)
driver = root/"driver"
main = (live/"main.rs").read_text().replace('        "check" => {', '''
        "phases" => {
            replay_try_from_phases(&frames, cpu_ns);
            let mut sum = [0i64; 4];
            for _ in 0..3 {
                for (s,n) in sum.iter_mut().zip(replay_try_from_phases(&frames, cpu_ns)) { *s+=n; }
            }
            for (name,n) in ["try_from","route","rows","append"].iter().zip(sum) {
                println!("{name}_ns={:.3}",n as f64/(3.*frames.len() as f64));
            }
        }
        "check" => {''')
manifest = driver/"Cargo.toml"
if args.confirm:
    with manifest.open("a") as f:
        f.write('\n[profile.release]\nlto=true\ndebug="line-tables-only"\n')
else:
    (driver/"main.rs").write_text(main)
    manifest.write_text(manifest.read_text().replace(str(live/"main.rs"), "main.rs"))
bindings = root/"game_module/src/module_bindings/mod.rs"
original = bindings.read_text()
tx = original.split("impl TryFrom<__ws::v2::TransactionUpdate> for DbUpdate", 1)[1].split("unknown =>", 1)[0]
arms = re.findall(r'"(\w+)"\s*=>\s*\{?\s*db_update\.append\(\s*__DbTableUpdate::(\w+)\(\s*(\w+)::parse_table_update', tx)
assert len(arms) == len(re.findall(r'"\s*=>', tx)) == 329
probe = (here/"probe.rs").read_text()
probe += "\nfn replay_table_id(name: &str) -> u16 {match name {\n" + "".join(
    f'"{name}" => {i},\n' for i,(name,_,_) in enumerate(arms)) + '_ => panic!("unknown"),}}\n'
probe += "fn replay_parse_id(id:u16,t:__ws::v2::TableUpdate)->Result<__DbTableUpdate,__sdk::Error>{Ok(match id {\n" + "".join(
    f'{i} => __DbTableUpdate::{variant}({module}::parse_table_update(t)?),\n'
    for i,(_,variant,module) in enumerate(arms)) + "_ => unreachable!(),})}\n"
if args.confirm:
    probe = ""
target = Path(os.environ.get("CARGO_TARGET_DIR", root/"target")).resolve()

def build(name):
    subprocess.run(["cargo","build","--offline","--release"],cwd=driver,
                   env={**os.environ,"CARGO_TARGET_DIR":str(target)},check=True)
    shutil.copy2(target/"release/sdk-live",root/name)

def patch(name, reverse=False):
    subprocess.run(["git","apply",*(["-R"] if reverse else []),here/f"{name}.patch"],
                   cwd=root/"sdk",check=True)

# Factor the 329 typed branches out of the hot string dispatch. Same full
# name comparison, row parser/error handling, and sorted append as the control.
start = original.index("            match &table_update.table_name[..] {", original.index("impl TryFrom<__ws::v2::TransactionUpdate> for DbUpdate"))
end = original.index("                unknown =>",start)
tail = original.index("\n            }",end) + len("\n            }")
routed = (original[:start] + "let parse: fn(__ws::v2::TableUpdate)->Result<__DbTableUpdate,__sdk::Error> = match &table_update.table_name[..] {\n" +
    "".join(f'"{n}" => |raw| Ok(__DbTableUpdate::{v}({m}::parse_table_update(raw)?)),\n' for n,v,m in arms) +
    original[end:tail] + ";\ndb_update.append(parse(table_update)?);" + original[tail:])
signature = "    fn append(&mut self, update: __DbTableUpdate) {\n"
assert original.count(signature) == 1
placement = original.replace(signature, signature + (here/"placement.rs").read_text())
for name in (["base", args.confirm] if args.confirm else ["base","row","route","both","placement"]):
    bindings.write_text((routed if name in ("route","both") else placement if name=="placement" else original) + probe)
    if name in ("row","both"):
        patch("single-row")
    build(name)
    if name in ("row","both"):
        patch("single-row", reverse=True)
