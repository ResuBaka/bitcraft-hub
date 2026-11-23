# Live XP state and history writes

## Results

**Keep the fixed-array writer for its client CPU savings.** It used less
CPU and had lower median commit latency in all 20 pairs across two runs.
The p95 result varied between runs.

| Per-pass median across ten paired runs | `VALUES` | `UNNEST` | Pairs lower with `UNNEST` |
|---|---:|---:|---:|
| Client-thread CPU, 42 transactions | 83.36 ms | 35.18 ms | 10/10 |
| p50 commit acknowledgment | 80.76 ms | 74.28 ms | 10/10 |
| p95 commit acknowledgment | 105.95 ms | 110.77 ms | 5/10 |

Median **within-pair CPU reduction: 57.4%** (55.4–59.5%). The per-pass p95
is the third-slowest of 42 flushes: the median within-pair change was 0.7%
slower, with individual pairs ranging from 13.6% faster to 22.2% slower.
[Per-pass CPU and individual commit latencies](nine-region.samples).

In the [confirmation run](pinned-server.samples), PostgreSQL was restricted
to CPUs 12–15, with the client still on CPU 11. Median within-pair CPU fell
**56.2%** (88.15 → 37.76 ms in the arm medians, 10/10 wins). Median p50
fell from **80.28 to 74.38 ms** (10/10); median p95 fell from **97.08 to
89.28 ms** (7/10). This supports a writer CPU improvement but does not
establish a reliable p95 gain on a shared host.

## What the replay measures

The live profile put about 0.85% of sampled API CPU in the XP database writer.
Its 60 transactions updated 56,164 state rows (median 938). For the replay we
used the last **42 consecutive transactions** from that same nine-region
window, where the number of state changes recorded by the XP trigger
matched the independent heap-WAL update count in every transaction.
The trigger can omit same-XP writes, so its rows alone cannot reconstruct
every transaction in the longer window.

| Per transaction | Total rows | Min | Median | Max |
|---|---:|---:|---:|---:|
| State upsert (all existing keys) | 39,050 | 901 | 930.5 | 955 |
| Explicit history append | 56,347 | 1,290 | 1,339.5 | 1,393 |

The read-only [export](capture.sql) groups rows by transaction ID. Trigger
rows supply the state input; explicit rows supply the observed timestamp,
player, skill, before/after XP, region and location (38,092 claim, 17,136
unclaimed, 1,119 unknown). Each list is in timestamp order with physical
tuple position as a tie breaker; all explicit timestamps in this capture are
distinct. Repeated observations of a key are retained.
The final observed XP for every key matches its state upsert. The fixture
contains private row data and is not checked in (JSONL SHA-256
`6946f4596f2970f1ad3775441e9ab6ca114c5d899ae9bc5055a186ddb2bbc9e6`).

Both writers start with the same owned state models and observed events.
The baseline converts models to active models, runs the original SeaORM
`insert_many(...).on_conflict(...).exec()`, then appends history with the
original 500-row `VALUES` builder. The candidate calls the production
writer: four typed arrays for the state upsert, eight for the complete
history append. This takes each captured flush from four write statements
to two. Both execute `BEGIN`, state first, history second, `COMMIT`.
The CPU and latency timers stop at commit acknowledgment. The comparison
includes client conversion/binding, database triggers, and round trips;
worker queues, deduplicating notification recipients and broadcasts are outside timing.
These are writer measurements, not whole-API CPU or client-visible websocket latency.

The [replay](replay.rs) uses fresh logged tables on a disposable PostgreSQL
18.3 / TimescaleDB 2.25.2 container over loopback, with fsync and synchronous
commit on. The [schema](schema.sql) includes both state indexes in addition
to the primary key, a daily history hypertable with both indexes, the location
check, the real state-history trigger, and the four continuous aggregate
definitions (no refresh or retention jobs). Each pass seeds the 1,371
affected keys from their first observed previous XP, rather than copying the
full database. The untimed seed uses the snapshot state statement on both
connections (warming that statement) and creates no trigger history.

One warmup per writer, then ten alternating-order pairs with a pinned
current-thread Tokio release client. Each pass checks all final state rows,
all 56,347 explicit history rows as a multiset of all eight fields, and
the 39,050 additional trigger-history rows by count. A separate PostgreSQL
constraint-failure test checks that a state update, state insert, their
trigger rows, and the explicit append all roll back together. The worker's
notification tests check player-before-claim ordering after commit and
no notifications on a failed history write.

## Reproduce

From `rust/api-server`, with a separate **disposable** TimescaleDB database
named `xp_replay_scratch` and the private fixture:

```sh
export XP_WRITE_FIXTURE=/private/xp-capture.jsonl
export XP_WRITE_TEST_URL=postgres://postgres@127.0.0.1:PORT/xp_replay_scratch
export CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_DEBUG=0
features=tokio-worker-queues,spacetimedb-tokio-websockets-rustls
cargo test --locked --release -p api --features "$features" --lib \
  live_xp_write_rolls_back -- --ignored
taskset -c 11 cargo test --locked --release -p api --features "$features" --lib \
  compare_live_xp_writes -- --ignored --nocapture > replay.log
python3 api/benches/live-progressive-writes/summary.py replay.log
```

For a new fixture, supply a padded timestamp interval and the first/last
complete transaction IDs to `psql -XqAt -v ON_ERROR_STOP=1 -v lo=... -v hi=...
-v first_xid=... -v last_xid=... -f capture.sql` on a **read-only** source.
Keep the output private. Compare per-transaction state counts against WAL
before using that interval; the benchmark expects all state writes to be
conflict updates and checks the state/history correspondence.
