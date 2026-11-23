# XP update CPU: claim lookup and leaderboard writes

The [live debug profile](../sdk-live/README.md) put the XP worker on 5.08% of
sampled user cycles, including claim lookup (1.15%) and immediate board
updates (1.01%). The following is a separate, single-region replay.

| XP handler variant | Thread CPU ns / update (median) | Paired wins vs control |
|---|---:|---:|
| Original boards, no search hints | 1,219 | — |
| Original boards, claim hints | 1,141 | 8/8 |
| New boards, no search hints | 1,125 | 8/8 |
| **New boards and claim hints** | **1,088** | **7/8** |

Median **within-block** reduction for the combination: **10.1%**. Each run
produced the same 37,649 ordered XP history entries (checksum of player, skill,
previous/new XP and claim: `b7e623ea527650bd`, excludes wall-clock timestamps).
[Per-process results](samples.jsonl).

* **Claims:** 36,871 of 37,649 observations revisited the previous tile for
  that player, but only 58 repeated the previous observation globally. The 233
  players visited 738 different tile/interior keys. Large immutable compact
  bases now keep 512 coordinate-to-lower-bound hints (4 KiB per base). A hit
  avoids a binary search. Readiness, live edits, duplicate-coordinate ambiguity
  and interior resolution are still evaluated. Rebuilds create fresh hints.
* **Boards:** one hash-map entry lookup coalesces each pending score change.
  Only the transition from an empty pending map needs to wake the publisher;
  after the publisher takes that batch the next change wakes it again.
  Immediate scores and the existing publication interval are unchanged.

**Isolated timings:** four alternating-order pairs on core 11 measured claim
lookup at 103.6 → 63.2 ns per captured position. For context, the original
skill-board resolution and immediate write took 90.3 ns per captured write.
On the existing 3.29M-tile synthetic *random* lookup benchmark, present
lookups measured 316 → 318 ns; absent lookups regressed from 29.5 to 32.7 ns.
The hint helps workloads with locality, not arbitrary coordinates.

## Reproduce

Use the private [region-14 wire fixture](../sdk-decode/README.md). The
ignored [replay](replay.rs) decodes snapshots and transactions outside timing.
It pairs old/new XP by player, applies mobile rows in wire order (end-of-
transaction position), and uses captured skill descriptions, starting signed-in
times and initial claim geography. This is a deterministic approximation of
two asynchronous workers, not a reconstruction of their live scheduling.

Build the API lib test binary twice in a disposable checkout, with the same
replay in both. For the original board binary use `src/leaderboard/compact.rs`
from crafting-list `5a4d91b5`; for the other use the current implementation.
Under `rust/api-server`:

```sh
cargo test --locked --offline -p api --lib --no-run \
  --features tokio-worker-queues,spacetimedb-tokio-websockets-rustls
# Save the printed executable path to a separate file after each build.
python3 api/benches/xp-updates/measure.py "$XP_WIRE" 11 "$ORIGINAL_BIN" "$NEW_BIN"
```

The runner rotates and reverses the four combinations over eight blocks. A
test-only switch removes the hints from an otherwise identical snapshot. Each
fresh process performs one warmup and seven timed passes. The timed section
consumes already-owned rows through `handle_update`, including position changes,
immediate scores and queuing history in memory. No external subscribers or DB
flushes; the independent ranking publisher can run but is not charged to
the measured worker thread. These are optimized **test-profile** results, not
whole-service CPU or release deployment measurements.
