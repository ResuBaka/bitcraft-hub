# Craft-hit project cache: region-14 replay

**93.5% less thread CPU per project operation** on this capture. The snapshot
seeded 1,064 projects; the replay processed 100 inserts, 100 deletes, and 48,601
paired updates. Every update's projection matched the cached value, so all
48,601 update writes were eliminated. All 24,327 craft-hit activity rows matched.

| Project update listener | Median ns/op | Range across nine samples |
|---|---:|---:|
| Unconditional cache write | 20.676 | 19.691–20.953 |
| Compare old/new projection | 1.346 | 1.224–1.887 |

Ryzen 9 9950X3D, Linux CPU 4, rustc 1.100.0; 21 passes (1,024,821 project
operations) per sample. These are listener timings, not whole-server CPU savings.

This A/B measures project listener CPU on the private [SDK decode capture](../sdk-decode/README.md#checks-and-reproduction).
Each pass starts from the captured project snapshot and applies the same inserts,
deletes, and primary-key-paired updates in callback order. The control always
inserts the new projection; the candidate calls production `Context::update_project`.
Both run in one optimized test binary. Each has one warmup; the nine timed pairs
alternate order. Units are thread CPU ns/project operation, excluding snapshot
seeding, wire decoding, SDK dispatch, craft-hit construction, and database writes.

Before timing, the replay checks that the cached projection matches the old row
for every update, the affected entries agree after every operation, and every
captured craft hit produces the same full activity row (using a common timestamp).
The regular unit test exercises building, recipe, count, and ID changes, which
the capture does not cover.

From `rust/api-server` on Linux:

```bash
export CRAFT_WIRE=/path/to/private/region14.wire
opts=(--locked -p api --lib --features tokio-worker-queues,spacetimedb-tokio-websockets-rustls
      --config 'profile.test.package.api.opt-level=3')
cargo test "${opts[@]}" crafting::events::tests
taskset -c 4 cargo test "${opts[@]}" -- --ignored --exact \
  crafting::events::replay::captured_updates --nocapture
```
