# Live inventory flushes: `insert_many` vs snapshot `UNNEST`

**The fixed-shape writer used 88.3% less client CPU on this replay of the
nine-region API's live flushes.** Median p95 time to PostgreSQL acknowledgment
fell from 30.40 to 19.16 ms. Both metrics favored the fixed writer in all
ten paired blocks. This is writer performance, not a measured whole-API gain.

| Per pass (49 flushes) | Live `insert_many` | Snapshot `UNNEST` |
|---|---:|---:|
| Client-thread CPU | 404.84 ms | 47.36 ms |
| Client CPU per 1,000 rows | 36.79 ms | 4.30 ms |
| Total time awaiting/completing writes, including statement construction | 1,194.91 ms | 778.34 ms |
| Median flush | 23.65 ms | 15.30 ms |
| p95 flush | 30.40 ms | 19.16 ms |

### Workload and controls

- Recorded 100 seconds of steady traffic on 2026-11-23, directly from the
  live `inventory` table. The 49 consecutive complete upsert transactions
  contained **11,005 rows** across nine regions, with **232,996 pockets**.
  Batch sizes min/p50/p95/max: **178/220/282/294**. There were **115 inserts
  and 10,890 conflict updates**, with 38 different `VALUES` statement sizes.
- The [read-only capture](capture.py) uses heap WAL to identify touched tuple
  locations and SQL to fetch the matching **CTID and xmin**, rather than
  scanning the inventory table on each poll. It omits the final possibly
  partial transaction. An independent continuous WAL scan confirmed the sizes
  and insert/conflict counts. The reproduction script also checks its chunked
  scan against a continuous scan. Raw row payloads are private.
- The [replay](replay.rs) ran on a separate, disposable PostgreSQL 18.3
  instance over loopback with fsync and synchronous commit enabled. It uses
  a logged table, the live column types, primary key and all four secondary
  indexes (including the duplicate owner index present on the source).
  It seeds the 1,496 affected pre-existing rows, not the full table, and
  restores four intervening deletes where needed to reproduce later inserts.
  It preserves row order within each flush.
- The timer starts with the same owned `Vec<Model>` and stops at acknowledgment.
  Baseline: convert to active models, build the same SeaORM `insert_many`
  with `ON CONFLICT` and `.exec()` as the former live worker. Candidate: call the
  existing snapshot `write_changes`, serialize pockets directly to JSON text
  and bind seven arrays; no returned IDs. **No flush split at the 1 MiB byte
  cap**: each writer executed one upsert per flush. This measures the two
  writer paths together, not SQL syntax in isolation.
- One warmup per writer, then ten alternating-order pairs on core 11 in a
  release build. Each warmup read back **every batch** and compared all fields
  including pockets. Each measured pass checked the final table by primary
  key. CPU uses the current-thread Tokio client's thread clock. Acknowledgment
  latency includes server work, but PostgreSQL CPU was not measured separately.
  Initial reconciliation, changelog, queue waits, and notifications are outside
  the test. [Per-flush latencies and per-pass CPU](nine-region.samples).

### Reproduce

From `rust/api-server`, with Linux, PostgreSQL 18 and access to a steady
single inventory writer. Use a **disposable** database for the replay:

```sh
bench=api/benches/live-inventory-writes
python3 "$bench/capture.py" SOURCE_CONTAINER SOURCE_DB 100 "$private/capture.json.gz"
gzip -dc "$private/capture.json.gz" > "$private/capture.json"
export INVENTORY_LIVE_FIXTURE="$private/capture.json"
export INVENTORY_LIVE_TEST_URL=postgres://postgres@127.0.0.1:5432/disposable
CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_DEBUG=0 \
  taskset -c 11 cargo test --locked --release -p api \
    --features tokio-worker-queues,spacetimedb-tokio-websockets-rustls \
    --lib compare_live_inventory_writers -- --ignored --nocapture > replay.log
python3 api/benches/live-progressive-writes/summary.py replay.log
```

The live DB worker now receives coalesced inventory models and executes the
fixed-shape writer directly. A one-statement flush uses autocommit; a flush
that exceeds the byte cap executes all its chunks in one transaction.
A [confirmation replay using that production function](production-writer.samples)
on the same capture measured **402.91 → 47.44 ms** client CPU and **29.56 →
19.23 ms** p95 flush time (10/10 pairs for both). A separate PostgreSQL
constraint-failure test forces three chunks and verifies that neither an update
nor an insert from earlier chunks survives a failure in the third.
