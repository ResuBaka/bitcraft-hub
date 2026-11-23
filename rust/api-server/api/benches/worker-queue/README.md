# Worker queue shutdown checks

In the two-minute live debug API profile, cancellation-token `is_cancelled`
accounted for 3.54% of sampled user cycles; 1.60% was directly under worker
channel sends/receives. There were 302,901 accepted/received messages. These
percentages overlap. The benchmark isolates the queue, not the entire API.

Producers now share a single native sender behind the existing send lock.
Shutdown takes and drops that sender under the same lock. Successful sends
precede the seal, and the backend wakes idle workers and drains to EOF without
consulting a second cancellation signal, even with worker-owned sender clones.

| Workload | Tokio before → after, CPU ns/message | Kanal before → after |
|---|---:|---:|
| Ready send + receive | 119.3 → 46.8 | 113.4 → 41.7 |
| `recv_many(64)`, singleton | 133.3 → 50.1 | 130.9 → 50.3 |
| `recv_many(64)`, burst of 32 | 54.0 → 41.8 | 48.3 → 31.8 |
| Send/yield/receive on one runtime thread | 618.5 → 520.1 | 626.8 → 523.4 |
| Two producer threads, one receiver draining up to 64 | 341.6 → 265.3 | See below |

Medians of eight alternating-order pairs pinned to core 7 (12 pairs on cores
5,6,7 for the concurrent test, CPU summed across all three threads). Each
pass checks 262,144 messages; the first pass is warmup. All 12 concurrent
Tokio pairs favored the change, with a median paired CPU reduction of 26.8%.
[Samples](samples.jsonl). Times exclude sockets, payload decoding, database
writes, and application callbacks.

The concurrent stress test also exposed a stall on the **original** Kanal
adapter: only 2/12 runs completed within two seconds. Dropping the token alone
didn't resolve it. Kanal now drains the sync queue directly unless there is
an outstanding async receive to finish first. An empty speculative drain no
longer registers a no-op waker. The final adapter completed 24/24 runs
within the same deadline, preserving per-producer FIFO through shutdown.
No comparative concurrent speedup is reported for Kanal.

## Reproduce (Linux)

Build a baseline and a candidate lib-test executable with the same
[bench functions](replay.rs). The baseline uses `worker_channel.rs`,
`worker_channel_kanal.rs` and `shutdown.rs` from `e75ebb23` with only the
test-module include from the candidate added. Run one backend at a time.
For the API's Tokio backend use `--features tokio-worker-queues,spacetimedb-tokio-websockets-rustls`;
for Kanal omit `tokio-worker-queues`. Save each executable separately.

```sh
cargo test --release --locked -p api --lib --no-run --features "$FEATURES"
python3 api/benches/worker-queue/measure.py "$BASE" "$NEW" bench_worker_queue_hot_paths 7 8
python3 api/benches/worker-queue/measure.py "$BASE" "$NEW" bench_concurrent_worker_queue 5,6,7 12
```

The recorded measurements built the three production source files and the
same bench functions as an isolated release-LTO lib test with the workspace
dependency versions. The regular non-ignored integration tests also ran in
the API crate on both backends.
