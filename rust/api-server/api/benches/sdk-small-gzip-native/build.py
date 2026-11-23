"""Prepare the isolated f22 SDK and build each replay arm with the same driver."""
import os
import hashlib
from pathlib import Path
import shutil
import subprocess
import sys

here = Path(__file__).resolve().parent
out = Path(sys.argv[2]).resolve()
if sys.argv[1] == "prepare":
    subprocess.run([sys.executable, here.parent / "sdk-live/prepare.py",
                    sys.argv[3], out, "--revision", "f22bb3201"], check=True)
    bindings = (out / "game_module/src/module_bindings/mod.rs").read_bytes()
    assert hashlib.sha256(bindings).hexdigest() == "b68926f6f27e6dcfff16d67294b053af09eb1c3a05cde0303697c1b15cf80bc6"
    sys.exit()
sdk = out / "sdk/sdks/rust/src"
driver = out / "driver"
candidate = sys.argv[1] == "candidate"
if candidate:
    subprocess.run(["git", "apply", here / "small-gzip.patch"], cwd=out / "sdk", check=True)
else:
    manifest = driver / "Cargo.toml"
    manifest.write_text(manifest.read_text().replace(str(here.parent / "sdk-live/main.rs"), str(here / "main.rs"))
                        + '\nsha2="0.10"\n[profile.release]\nlto=true\ndebug="line-tables-only"\n')
    with (sdk / "db_connection.rs").open("a") as f:
        f.write("""
impl<M: SpacetimeModule> DbContextImpl<M> {
    pub fn replay_message(&self, msg: ws::v2::ServerMessage) {
        self.process_message(match msg {
            ws::v2::ServerMessage::InitialConnection(m) => ParsedMessage::IdentityToken(m.identity, m.token, m.connection_id),
            ws::v2::ServerMessage::SubscribeApplied(s) => ParsedMessage::SubscribeApplied {
                query_set_id: s.query_set_id, initial_update: M::DbUpdate::parse_initial_rows(s.rows).unwrap(),
            },
            ws::v2::ServerMessage::TransactionUpdate(t) => ParsedMessage::TransactionUpdate(M::DbUpdate::parse_update(t).unwrap()),
            _ => unreachable!(),
        }).unwrap();
    }
}
""")

# Benchmark-only access to the same per-connection decoder used by the socket.
lib = sdk / "lib.rs"
base = """#[derive(Default)]
        pub struct ReplayDecoder;
        impl ReplayDecoder {
            pub fn decode(&mut self, bytes: &[u8]) -> Result<ServerMessage, String> { streaming(bytes) }
        }
        """
if not candidate:
    lib.write_text(lib.read_text().replace("pub fn buffered(", base + "pub fn buffered("))
else:
    lib.write_text(lib.read_text().replace(base, """#[derive(Default)]
        pub struct ReplayDecoder(crate::native_small_gzip::SmallGzip);
        impl ReplayDecoder {
            pub fn decode(&mut self, bytes: &[u8]) -> Result<ServerMessage, String> {
                self.0.try_decode(bytes).unwrap_or_else(|| crate::streaming_decompression::decode(bytes)).map_err(|e| e.to_string())
            }
            pub fn hit(&mut self, bytes: &[u8]) -> bool { self.0.try_decode(bytes).is_some() }
        }
        """))
flags = ["--features", "spacetimedb-sdk/native-small-gzip"] if candidate else []
env = {**os.environ, "CARGO_TARGET_DIR": str(out / "target")}
subprocess.run(["cargo", "build", "--bin", "sdk-live", "--release", "--offline", *flags], cwd=driver, env=env, check=True)
shutil.copy2(out / "target/release/sdk-live", out / sys.argv[1])
if candidate:
    with (driver / "Cargo.toml").open("a") as manifest:
        manifest.write(f'\n[[bin]]\nname="coverage"\npath="{here}/coverage.rs"\n')
    subprocess.run(["cargo", "build", "--bin", "coverage", "--release", "--offline", *flags], cwd=driver, env=env, check=True)
