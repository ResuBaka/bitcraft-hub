# Live PostgreSQL upserts: `progressive_action_state`

**Live policy: use `UNNEST` at 144 or more distinct pending upserts; keep
`insert_many` below that.** One flush in the nine-region capture had 144
rows; the other 87 had 412–486. The cutoff is the observed floor, not an
estimated crossover. A newer live sample had a median of 538 rows per flush;
only 0.70% of rows were inserts. The
[worker](../../src/crafting/bitcraft.rs) chooses after coalescing; repeated
events and pending deletes do not inflate the count.
Either path writes the upserts in one statement. Deletes still follow separately.

**No small-batch latency win.** In a live region-13 capture, 38 flushes had
one row and 15 had two. Replaying them with `UNNEST` saved about **19 µs of
client CPU per flush** but did not reduce median or p95 time to acknowledgment.
Keep `insert_many` for small writes on this evidence. The
[3,000-row snapshot comparison](../regional-snapshot-writes.md) answers a
different question.

| Region 13: 53 flushes, all 68 rows conflict updates | Live `insert_many` | Fixed `UNNEST` | Pairs favoring `UNNEST` |
|---|---:|---:|---:|
| Total writer wall time | 568.02 ms | 574.62 ms | 5/12 |
| Client-thread CPU | 9.06 ms | 8.06 ms | 11/12 |
| Flush p50 | 9.51 ms | 9.58 ms | 4/12 |
| Flush p95 | 18.94 ms | 19.12 ms | 3/12 |

The running nine-region API supplied a useful contrast: **88 flushes, 144–486
rows (median 472)**, 311 inserts and 40,433 conflict updates. Here `UNNEST`
cut total write time **10%** (1,702 → 1,529 ms; faster in 9/12 pairs) and
client CPU **52%** (133.46 → 64.50 ms; 12/12). `VALUES` needed 41 distinct
statement shapes in this capture, versus just two in region 13. These two
captures do not locate a row-count threshold or establish an end-to-end API
speedup.

### Replay after integrating the live writer

Rerunning the same fixtures against disposable PostgreSQL 18.3, with the
current ORM arm and 12 alternating pairs, confirmed the size distinction:

| Fixture | Writer CPU, `VALUES` → `UNNEST` | Total write time, `VALUES` → `UNNEST` | Flush p50 / p95, `VALUES` → `UNNEST` |
|---|---:|---:|---:|
| Nine regions | 137.57 → 68.45 ms | 1,753 → 1,651 ms | 17.99 / 26.83 → 17.23 / 24.95 ms |
| Region 13 | 7.84 → 7.26 ms | 696 → 730 ms | 10.20 / 22.29 → 10.69 / 22.65 ms |

On the nine-region fixture `UNNEST` used **50% less writer CPU** (12/12
pairs) and **6% less wall time** (9/12). On region 13 it won wall time in
only 3/12 pairs. These are same-host writer measurements; the newer
538-row median is live workload evidence, not a replay input.

## Inputs and measurements

* On 2026-11-23, captured 180 s from the running local API's database. For
  small batches, ran a second instance subscribed to `idc-13` as its only
  region and captured 144 s after startup (six seed rows). Both used the
  normal 2 s flush window. Neither fixture splits a larger flush into pieces.
  WAL row counts matched all 53 and 88 captured upsert transactions.
* Replayed on a separate, same-host PostgreSQL 18.3 instance over TCP, with
  `fsync` and `synchronous_commit` on, one warm connection, and no competing
  writers on that instance. Release build, client pinned to core 11 on a
  Ryzen 9 9950X3D. One warm-up per arm, then 12 paired blocks alternating
  which writer goes first. Reported numbers are medians across blocks;
  percentiles are computed within each pass. CPU is Linux thread CPU on the
  single-thread Tokio replay, excluding PostgreSQL. Absolute acknowledgment
  latency depends on storage and network.
* [Small fixture](region13.json.gz), [small samples](region13.samples),
  [nine-region sizes and samples](nine-region.samples). Run
  `python3 summary.py region13.samples` in this directory for the per-size
  breakdown. The larger row capture was kept locally.

## What was timed

Each timed flush starts with the same coalesced `PendingActions` map and ends
when PostgreSQL acknowledges the upsert. In the original measurements above,
the baseline converted to ActiveModels and called `insert_many(iter().cloned())`,
updating all ten non-key columns on conflict, `.exec()` with `RETURNING`. The
candidate drained the same models into a reusable vector and called the existing
`Writer::Fixed(snapshot_write::progressive_actions)`: eleven typed array binds,
the same conflict updates, no returned IDs. This compares the two usable
writers, including binding, transfer and commit. It does not attribute the
difference solely to the SQL syntax.

The current replay forces each arm through the live worker's write path so
the comparison stays meaningful after rollout. Both now drain the coalesced
models into a reusable vector; the ORM arm converts the drained models to
ActiveModels without cloning them. The fixed arm uses the same
[statement](../../src/websocket/snapshot_write/regional.rs) as snapshot
reconciliation.

Both arms use the same key iteration order, starting rows, interleaved deletes,
primary key and two secondary indexes (building and owner). Deletes restore
the observed conflict mix but are outside the timer. The warm-up reads back
every flushed row, including JSONB `lock_expiration`; every pass verifies the
entire final table. Queue waits, callbacks and notifications are outside the
measurement.

Only four columns changed during conflict updates in the nine-region
fixture: `progress`, `lock_expiration`, `preparation` and `last_crit_outcome`.
As a separate correctness check, the `progressive_rows` PostgreSQL test varied
all ten non-key columns on 12,019 rows. The ORM and fixed writers produced
identical PostgreSQL row digests after both insert and update.

## Reproduce

On Linux, from `rust/api-server`, use a **disposable** PostgreSQL database
for timing. The ignored replay creates a private logged schema.

```sh
bench=api/benches/live-progressive-writes
gzip -dc "$bench/region13.json.gz" > /tmp/region13.json
export LIVE_WRITER_CAPTURE=/tmp/region13.json
export LIVE_WRITER_TEST_URL=postgres://postgres@127.0.0.1:5432/bench
CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_DEBUG=0 \
  cargo test --release --locked -p api \
  --features tokio-worker-queues,spacetimedb-tokio-websockets-rustls \
  --lib compare_progressive_writers -- --ignored --nocapture > replay.log
python3 "$bench/summary.py" replay.log
```

For the full-column check, use the same disposable URL in
`SNAPSHOT_WRITER_TEST_URL` and replace `compare_progressive_writers` with
`progressive_rows`. Run with `SNAPSHOT_WRITER_BASELINE=0` and `=1` and
compare the `snapshot_digest` lines on stderr.

For a fresh capture, wait for the API's regional startup to complete.
Run `python3 "$bench/capture.py" capture.json.gz 180 -- psql -XqAt "$SOURCE_URL"`
against the source database (PostgreSQL 18). It polls committed versions into
a session-local temp table, groups changes by inserting transaction (`xmin`),
and records deletes and the seed state. Entity, owner and building IDs are
relabeled consistently. Polling alone could miss a version overwritten between
reads: save the script's stdout, bracket it with `pg_current_wal_lsn()` readings,
and check against `pg_waldump -r Heap -R tablespace/db/relfile -s start -e end`
for this table. `check-wal.py capture.stdout table.wal` checks every upsert
transaction between the first and last captured XIDs, including each one's
insert vs. conflict-update count. These captures are of a single live table
writer, in steady state, without XID wraparound.
