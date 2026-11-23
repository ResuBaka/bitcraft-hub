# Live PostgreSQL upserts: `progressive_action_state`

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
when PostgreSQL acknowledges the upsert. The baseline calls the worker's
`flush`: convert to ActiveModels, `insert_many(iter().cloned())`, update all
ten non-key columns on conflict, `.exec()` with `RETURNING`. The candidate
drains the same models into a reusable vector and calls the existing
`Writer::Fixed(snapshot_write::progressive_actions)`: eleven typed array binds,
the same conflict updates, no returned IDs. This compares the two usable
writers, including binding, transfer and commit. It does not attribute the
difference solely to the SQL syntax.

Both arms use the same key iteration order, starting rows, interleaved deletes,
primary key and two secondary indexes (building and owner). Deletes restore
the observed conflict mix but are outside the timer. The warm-up reads back
every flushed row, including JSONB `lock_expiration`; every pass verifies the
entire final table. Queue waits, callbacks and notifications are outside the
measurement.

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
