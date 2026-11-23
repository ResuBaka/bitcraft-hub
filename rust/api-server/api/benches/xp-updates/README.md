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

## Skill-description reads

The subsequent live debug profile attributed 1.66% of samples to XP skill
description access, including 1.28% in read lock/unlock leaves. On the same
region-14 replay, using one read guard per changed player update reduced worker
CPU in **8/8** alternating-order pairs:

| Skill definition lookup | Median thread CPU ns/update |
|---|---:|
| Per-skill DashMap reads (`e75ebb23`) | 1,128 |
| One shared map read per update | 956 |

Median **within-pair reduction: 14.1%**. Both variants used the newer boards
and claim hints; all seven measured passes of all 16 processes produced 37,649
ordered history entries with digest `b7e623ea527650bd`. [Per-process results](skill-samples.jsonl).
The history digest does not cover board totals or notifications; the XP worker
tests check those, including changes in skill name and category between updates.

The worker reads full definitions from one `RwLock<HashMap>` so categories
and names are consistent for both sides of an XP update. Skill edits take the
write lock; successful complete skill snapshots replace the map, removing
missing skills. XP does not hold the guard across a DB or channel await.

To reproduce, build and save the optimized API lib test binary as above at
`e75ebb23` and with the skill-map change applied. Use each revision's own
`replay.rs` (the fixture loader uses the matching cache API), then run:

```sh
python3 api/benches/xp-updates/measure.py "$XP_WIRE" 11 "$CONTROL_BIN" "$NEW_BIN" --skills
```

## One changed skill

The later live trace attributed 6.02% inclusive CPU to `handle_update`.
In the region-14 fixture, all 37,649 updates have 20 aligned skills, exactly
one changed quantity, and only 65 updates change the sum of levels.

**Keep the single-skill path.** Median worker-thread CPU fell from **848 to
651 ns/update**, lower in **8/8** alternating-order pairs (median within-pair
reduction **23.0%**, range 21.2–28.9%). One warmup and seven timed passes per
process, both variants pinned to logical CPU 11 on a Ryzen 9 9950X3D.
[Paired samples](single-samples.jsonl).

All 112 measured passes matched the 37,649-entry ordered history digest
`b7e623ea527650bd` (excluding observation time). Both subscribed runs matched
the per-update board score digest `947a974b0863ea79` and all **113,012**
ordered notification payloads (digest `6d5c24a6925b78e7`).

The worker checks the first difference and the entire remaining suffix before
using the single-skill path. Only the changed skill needs a before/after level
conversion. A level change triggers an absolute level sum; an XP change always
triggers an absolute XP sum and, for profession/adventure, the corresponding
absolute category sum. Category membership comes from the existing live
skill-description guard, including for unchanged skills. Neither an old board
score nor a per-player cache is used. Other row shapes take the general path,
with the same per-skill history, score writes and notification code.

To reproduce, build two release API lib test executables from the same working
tree, with the same `replay.rs` and only the handler change varied. The control
includes the one-guard skill lookup and the fixed-array XP/history writer.
From `rust/api-server`:

```sh
export CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_DEBUG=0
cargo test --locked --offline --release -p api --lib --no-run \
  --features tokio-worker-queues,spacetimedb-tokio-websockets-rustls
# Save the printed executable after each build.
python3 api/benches/xp-updates/measure.py "$XP_WIRE" 11 "$CONTROL_BIN" "$NEW_BIN" --single
```

The runner first checks an untimed subscribed replay. It seeds boards from
each player's first old row and publishes ranks once, so the two executables
can compare full payloads (including rank) deterministically. After each
update it hashes the immediate skill, global, level, category and XP/hour
scores and the ordered JSON notifications. Timing uses the existing separate
subscriber-free replay with mobile positions, claim lookup and queued history.
This is one region on a shared host, with no database flush or measurement of
production publisher timing, fallback frequency, or whole-service savings.
