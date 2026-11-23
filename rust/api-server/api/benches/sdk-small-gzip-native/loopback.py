"""Exercise the actual native socket with bad, valid and oversize gzip frames."""
import os
from pathlib import Path
import subprocess
import sys

here = Path(__file__).resolve().parent
out = Path(sys.argv[1]).resolve()
driver = out / "gzip-loopback"
driver.mkdir()
manifest = (out / "driver/Cargo.toml").read_text().split('\n[[bin]]\nname="coverage"')[0]
manifest = manifest.replace(str(here / "main.rs"), str(here.parent / "sdk-owned-initial/lifecycle.rs"))
(driver / "Cargo.toml").write_text(manifest)
(driver / "Cargo.lock").write_bytes((out / "driver/Cargo.lock").read_bytes())
env = {**os.environ, "CARGO_PROFILE_DEV_DEBUG": "0", "CARGO_INCREMENTAL": "0", "CARGO_TARGET_DIR": str(driver / "target")}
subprocess.run(["cargo", "build", "--offline", "--features", "spacetimedb-sdk/native-small-gzip"],
               cwd=driver, env=env, check=True)

js = (here.parent / "sdk-owned-initial/lifecycle.js").read_text()
old = "const send = (tag, ...parts) => ws.send(cat(byte(0), byte(tag), ...parts));"
assert js.count(old) == 1
js = 'import {gzipSync} from "node:zlib";\n' + js.replace(old, """
      const send = (tag, ...parts) => {
        // Legal trailing BSATN bytes force dynamic Huffman across these tiny
        // messages; the reducer response expands past the fast output limit.
        const pad = Buffer.alloc(tag === 6 ? 16384 : 1600);
        for (let i=0; i<pad.length; ++i) pad[i] = ((i%47)+(i%103))%31;
        const gz = gzipSync(cat(byte(tag), ...parts, pad));
        assert.ok(gz.length + 1 <= 4096);
        if (tag !== 6) assert.equal(gz[10]&7, 5);
        if (tag === 4) {
          const bad = Buffer.from(gz);
          bad[bad.length-8] ^= 1;
          ws.send(cat(byte(2),bad));
          ws.send(cat(byte(2),gz.subarray(0,-1)));
        }
        // Invalid trailing data outside the first member must be ignored.
        ws.send(cat(byte(2), gz, Buffer.alloc(4)));
      };""")
(driver / "loopback.js").write_text(js)
subprocess.run(["bun", driver / "loopback.js", driver / "target/debug/sdk-live"], check=True)
