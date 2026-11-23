# XP skill categories: full-handler release replay

**Keep the index.** On the captured region-14 XP stream, worker-thread CPU fell
from **664.14 to 475.60 ns/update**. The median within-pair reduction was
**27.6%** (23.7–31.9%); all eight alternating-order pairs favored the index.
[Samples](samples.jsonl) include all 112 timed passes and the subscribed parity results.

| Same 37,649 updates | Original descriptor map | Map + category index |
|---|---:|---:|
| Median thread CPU per capture pass | 25.00 ms | 17.91 ms |
| Ordered positive-XP history entries / pass | 37,649 | 37,649 |
| History digest / pass | `b7e623ea527650bd` | `b7e623ea527650bd` |
| Per-update immediate score digest, subscribed pass | `947a974b0863ea79` | `947a974b0863ea79` |
| Ordered notifications, subscribed pass | 113,012 | 113,012 |
| Notification payload digest | `6d5c24a6925b78e7` | `6d5c24a6925b78e7` |

## What changed

[SkillDefinitions](../../src/skill_descriptions/definitions.rs) keeps 64
`Option<i32>` category slots (512 bytes here) beside the full descriptor map,
under the existing lock. Missing slots and signed IDs outside `0..64` read
the original map; categories are never narrowed. Startup inserts, live edits
and deletions update both representations. A complete skill snapshot builds a
fresh pair and publishes it in one assignment only after reconciliation succeeds.

Only the category rescan for a single changed XP quantity uses the index. It
still sums the **absolute new stacks**, including unchanged skills using their
current definitions. The changed skill's full descriptor still determines its
name, individual board eligibility, notification and target aggregate. The
general multi-change/reordered path and score publication policy are unchanged.
There is no per-player index or new lock acquisition.

## Measurement and validation

The control was built from the pre-index **working tree**, including its
single-skill handler, not from clean HEAD (`a5344722`). Same lockfile
(SDK `f22bb320`), compiler (`rustc 1.100.0`), feature set and
[existing full XP replay](../xp-updates/replay.rs) for both arms. Both were
release lib-test executables, opt-level 3, LTO and debug info off, using the
system allocator. Linux, Ryzen 9 9950X3D, logical CPU **8** for every timed
process. Eight AB/BA pairs; one warmup plus seven measured passes per fresh
process. Each process contributes its median; percentages compare matched
processes, not the two pooled medians.

The private 62,737,115-byte fixture has SHA-256
`c5c7878915085400621d32396d72a8d93c37ca80e6e2fc13859a3613a3bb3997`. All
37,649 updates change one quantity among 20 aligned skills with IDs in 1–22:
752,980 category checks per pass. Timing starts after decoding and cloning
the input and includes the actual `handle_update`, position changes, claim
lookup, immediate board writes and queuing history. Every timed pass matched
the established history digest (player, skill, old/new XP, claim; observation
timestamps excluded). A separate untimed run seeds and publishes starting
ranks, subscribes to the XP topics, then checks all immediate skill, XP,
level, profession, adventure and XP/hour scores and ordered serialized payloads
after every update, including rank.

The [handler edge test](edges.rs) checks an unchanged skill moving categories,
renames, deletion of a definition with an existing board, stale aggregate
scores, dense IDs 0/63, sparse 64, negative and extreme signed IDs, category
257, and missing descriptors. It checks the actual ordered events and queued
history. The descriptor worker tests cover successful replacement, live
edits/deletes, a failed reconciliation after rows have been observed, and an
empty replacement with a succeeding or failing pre-snapshot flush. Existing
tests also cover XP decreases, category zero, reordered and unequal rows,
excluded players and debug-rank notifications. The release candidate passed
80 leaderboard/descriptor tests (five opt-in replays skipped in that suite);
the handler edge test and both descriptor-worker cases also passed against
the reconstructed plain-map control.

**Boundary:** This is subscriber-free worker CPU on one region, not API process
CPU, DB flush latency, publisher CPU, or a live-service speedup. End-of-
transaction positions approximate asynchronous worker ordering. The cost of
rare descriptor writes and sparse/unknown-ID traffic is not measured.

## Reproduce

From `rust/api-server` in a **disposable copy of the working tree**, build and
save the candidate, then use [control.patch](control.patch) to restore the plain
map and descriptor-based rescan, retaining the same edge tests:

```sh
export CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_DEBUG=0
features=tokio-worker-queues,spacetimedb-tokio-websockets-rustls
build() {
  cargo test --offline --locked --release -p api --lib --no-run \
    --features "$features" --message-format=json-render-diagnostics > build.json &&
    cp "$(jq -r 'select(.executable) | .executable' build.json | tail -1)" "$1"
}
build ./xp-index
patch -p1 < api/benches/xp-category-index/control.patch
build ./xp-control
for binary in ./xp-index ./xp-control; do
  "$binary" category_edges skill_descriptions::bitcraft::tests --test-threads=1
done
python3 api/benches/xp-category-index/measure.py "$XP_WIRE" ./xp-control ./xp-index
```
