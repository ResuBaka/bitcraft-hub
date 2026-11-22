# Passive craft and vault snapshot writes

These were the largest remaining changed-row writers in the inventory follow-up
trace. At the same global peak they accounted for 43.0 MB and 39.2 MB,
respectively. The new snapshot paths use one PostgreSQL `INSERT ... UNNEST ...
ON CONFLICT` shape per table, with no `RETURNING`. All columns have bounded
encoded widths, so the existing 3,000-row maximum already bounds payload size
(under 300 KiB per statement); they don't need inventory's per-row JSON size
estimator.

* **Passive craft:** the regional reconciler takes a writer for changed models;
  its default remains SeaORM for other callers. The pre-snapshot flush,
  comparisons and stale deletes still happen in the same order.
  Nine typed arrays include a slot-value array and a presence array: the
  binder needs non-null array elements, while the table distinguishes `NULL`
  from `0`. Timestamps are sent as `timestamptz`.
* **Vault collectibles:** keep the specialized comparison, including duplicate
  detection, oversized vault partitioning and end-of-snapshot deletion. Only
  its persistence step changes: five scalar arrays, upserted on `(entity_id, id)`.
  Live-update writers retain their previous behavior.

## Controlled PostgreSQL replay

One process per table/variant, identical synthetic fixed-width rows, dedicated
PostgreSQL 18 instance and fresh logged table with its primary key. Insert
12,019 rows in five statements, then change every row and upsert on the existing
key in five more. After **each** pass, stream back the entire table and compare
every column. This exercises moved regions, microsecond timestamps, `NULL`/zero/
populated slots, composite keys and a short final statement.

| Writer | Heaptrack peak¹ | Allocation calls¹ | Insert / update² | Insert / update WAL³ |
| --- | ---: | ---: | ---: | ---: |
| Passive craft, SeaORM `VALUES` | 7.94 MB | 82,439 | 213 / 210 ms | 2,757,032 / 3,840,488 B |
| Passive craft, `UNNEST` | **2.67 MB** | 34,429 | 186 / 189 ms | 2,757,056 / 3,840,464 B |
| Vault collectibles, SeaORM `VALUES` | 4.72 MB | 50,077 | 184 / 198 ms | 2,445,608 / 3,543,408 B |
| Vault collectibles, `UNNEST` | **1.50 MB** | 26,080 | 173 / 188 ms | 2,445,608 / 3,543,400 B |

1. Decimal MB; entire test process including row verification, one instrumented
   run per variant. The baseline uses the previous execution mode for each
   table (`exec` for craft, `exec_without_returning` for vault).
2. Median writer time of three uninstrumented runs per variant, including
   DB execution and commit. Input generation and verification are outside the
   timer. Wall times are from a shared build host.
3. Median `pg_current_wal_insert_lsn()` delta per pass. Production secondary
   indexes, concurrent writes and replica lag are outside this replay.

The full API library suite passed on the writer-only candidate (396 passed)
and on the shared working tree with concurrent SDK changes (398 passed, 15
ignored). The opt-in PostgreSQL vault reconciliation test also passed,
including its oversized-vault, region-move and stale-key cases.

## Whole-server Heaptrack comparison

Two serialized four-minute ingestion runs, on fresh copies of the same
claim-sorted database, with the same SDK and release/system-allocator build
settings. Baseline: `4fecb67a`; candidate: that revision plus these writer
changes, excluding concurrent SDK work.

| Metric (decimal units) | Before | After |
| --- | ---: | ---: |
| Peak outstanding heap, entire API | 606.66 MB | 546.23 MB |
| Peak outstanding heap from passive craft snapshot upsert stacks | 52.40 MB | 2.16 MB |
| Peak outstanding heap from vault snapshot upsert stacks | 44.68 MB | 1.20 MB |
| Peak RSS, including profiler overhead | 1.59 GB | 1.61 GB |

The whole-process heap peak was **60.43 MB (10%) lower** in the candidate
run; profiled peak RSS did not improve. Each writer's maximum is measured
independently, not added to derive the process peak. At the actual process
peak, those two stacks held 88.65 MB together before and 0.06 MB after.
Both runs reached all ten connections, reconnected one region, observed 20
inventory/vault completion markers, and drained gracefully without a reported
passive-craft or vault snapshot failure. Other workers had one pool timeout
before and four after, and upstream traffic was live; the controlled replays
above are the better basis for claims about throughput or WAL.

The next largest **among the remaining generic snapshot-upsert stacks** were
traveler tasks (17.48 MB local peak) and XP (15.50 MB). Local raw captures,
summaries and per-table attribution are under
`rust/api-server/target/snapshot-writers-profile-aik5jx/`.
See [remaining regional and custom snapshot writers](regional-snapshot-writes.md)
for the follow-up migrations.

## Reproduction

Reproduce from `rust/api-server` against a disposable database that permits
schema creation. The tests create a private logged schema and drop it on
success. Set `SNAPSHOT_WRITER_TEST_URL` and run:

```sh
cargo test --locked -p api --features tokio-worker-queues,spacetimedb-tokio-websockets-rustls \
  --lib websocket::snapshot_write::replay -- --ignored --nocapture --test-threads=1
```

Repeat with `SNAPSHOT_WRITER_BASELINE=1`. For Heaptrack, build with `--no-run`
and profile the test executable directly, filtering to `passive_craft_rows`
or `vault_collectible_rows` and passing `--ignored --nocapture`. Use a dedicated
DB instance for WAL deltas.
