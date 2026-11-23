# Player activity index: region-14 replay

The growable vector used **23.4% less thread CPU per index operation** than the
nested `HashMap` on captured updates. It won all nine alternating-order pairs.

| Implementation | Median ns/op | Range across nine samples |
|---|---:|---:|
| Previous nested map (`e75ebb23`) | 48.785 | 46.698–50.238 |
| Production vector, initial capacity two | 37.361 | 36.601–39.249 |

The capture starts with 2,814 action rows for 1,407 players, two each.
118,373 subsequent action transactions touched 259 players: 148,738 paired
updates, six inserts, ten deletes, and no changes of owner. Before timing,
the replay checks complete row equality (ignoring order) on the initial
snapshot, for the affected player after each operation, and across the full
final index. All checks passed.

Each pass starts from the original snapshot and applies the 148,754 captured
operations in order. Timing includes the row clone on upsert, region write
lock, and index mutation. Wire decoding, primary-key pairing, snapshot seeding,
verification, and index teardown are outside timing. This measures index CPU;
it does not measure end-to-end listener or server CPU. Both implementations
run in the same test binary, with one warmup pass each and seven passes per
sample (1,041,278 operations). Ryzen 9 9950X3D, Linux CPU 4, rustc 1.100.0.

## Reproduce

Use the private `.wire` input described in [SDK decode](../sdk-decode/README.md#checks-and-reproduction)
(SHA-256 `c5c7878915085400621d32396d72a8d93c37ca80e6e2fc13859a3613a3bb3997`).
The replay reads it in memory and prints only aggregate counts and timings.
From `rust/api-server`, on Linux:

```bash
export ACTIVITY_WIRE=/path/to/private/region14.wire
opts=(--locked -p api --lib --features tokio-worker-queues,spacetimedb-tokio-websockets-rustls
      --config 'profile.test.package.api.opt-level=3')
cargo test "${opts[@]}" --no-run
taskset -c 4 cargo test "${opts[@]}" -- --ignored --exact \
  player_state::activity::replay::captured_updates --nocapture
```
