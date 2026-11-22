# Inventory snapshot write experiment

The shipped writer keeps the 3,000-row comparison limit, splits changed rows
at a configurable estimated **1 MiB of bind payload**, then upserts with one
SQL shape and seven array parameters. The pockets column is sent as `text[]`
of compact JSON, cast per row to `jsonb` in PostgreSQL. This keeps SeaORM's
connection pool and execution callback without constructing/cloning a JSON
object tree per pocket. There is no staging table.

## Controlled PostgreSQL replay

The byte cap was implemented and tested with the original `insert_many`
(`VALUES`) writer before replacing it with `UNNEST`. Each run used the same
6,000 inventory rows (166,002 pockets) in entity-ID order from the prior
benchmark fixture: 96,105 empty slots, 68,265 item stacks and 1,632 cargo
stacks. The first pass inserted all rows into an empty logged table.
The second pass changed all six non-key columns where applicable and conflicted
on every ID. The test streamed the resulting table back and checked every row
against the expected model, including pockets.

| Writer | Statements per pass | Heaptrack peak¹ | Allocation calls¹ | Insert / update time² | Insert / update WAL³ |
| --- | ---: | ---: | ---: | ---: | ---: |
| Row cap only, `VALUES` | 2 | 284.10 MB | 7,065,260 | 796 / 628 ms | 4,625,672 / 5,160,344 B |
| Row + byte cap, `VALUES` | 16 | 39.22 MB | 7,068,403 | 967 / 890 ms | 4,626,232 / 5,152,776 B |
| **Same caps, `UNNEST`** | **16** | **8.66 MB** | **183,177** | **469 / 486 ms** | **4,626,208 / 5,152,776 B** |

1. Decimal MB of requested live heap; one Heaptrack run per variant. Includes
   fixture parsing, both writes and verification. Not a whole-server RSS estimate.
2. Median of three **uninstrumented** runs of the optimized test binary;
   wall clock inside the writer, including database execution and commit.
   Input parsing and verification are outside the timer. The row-only control
   used a nonbinding byte cap on the intermediate writer.
3. Median change in `pg_current_wal_insert_lsn()` across the corresponding
   pass on a dedicated PostgreSQL 18 instance, with a fresh logged table and
   its primary key per run. The real schema, its other indexes, concurrent
   writers, checkpoints and any replica lag are not represented by this test.

The byte cap cut the writer's peak at the cost of more round trips. At the
**same statement count**, `UNNEST` then cut peak heap another 78%, recovered
the throughput loss, and produced essentially the same WAL. These numbers
describe **changed rows**: the normal comparison still skips unchanged rows.
Restricting the allocation stacks to `inventory/snapshot_write.rs`, the writer
itself peaked at 283.68 MB, 35.84 MB and 5.32 MB respectively. With this
result there is no need to add another storage/write path via `COPY` unless
future profiles still attribute a material bottleneck to this writer.
An oversized row is never split across writes; a write error stops the
snapshot before stale-row deletion. Previously successful batches may already
have committed, as before.

## Whole-server Heaptrack check

Release build with the system allocator, real upstream, one global and nine
regional subscriptions. Each variant started on a fresh clone of the same
claim-sorted DB. The new captures ran for 240 s plus drain. The earlier
row-only capture ran for 360 s; its peak was already reached at ~128 s.
Live traffic and retries differ, unlike the controlled writer replay above.

| Writer | Process peak heap | Peak live bytes from snapshot-write stacks¹ | Live heap, median 200–235 s |
| --- | ---: | ---: | ---: |
| Earlier row-only `VALUES` | 872.83 MB | 354.07 MB | 244.51 MB |
| Row + byte caps, `VALUES` | 722.01 MB | 97.82 MB | 232.90 MB |
| **Same caps, `UNNEST`** | **628.80 MB** | **52.77 MB** | **233.29 MB** |

1. Groups are based on allocation stack, even if a driver/pool later retains
   the allocation. Intermediate/final captures use `inventory/snapshot_write.rs`;
   the earlier capture uses the original snapshot `insert_many` call site. The
   writer peak and process peak need not be simultaneous.

At the same limits, the final live run peaked **93 MB lower** than capped
`VALUES` (13%); the late median changed by less than 1 MB. This is mainly a
startup/reconnect improvement. Heaptrack reported 2.12 GB vs 1.52 GB peak
RSS, including profiling overhead; this is not a native mimalloc RSS benchmark.
Snapshot-write stacks accounted for 30.45 million allocation calls in the
capped `VALUES` run and 0.31 million with `UNNEST`.

Both new runs reached all ten connections, recovered one regional disconnect,
read 20 inventory/vault snapshot completion markers and drained gracefully.
Neither logged an inventory write failure. Capped `VALUES` also logged one
claim-worker DB pool timeout. The local raw traces, summaries and intermediate
writer source are in `rust/api-server/target/inventory-writer-profile-fq6sz8/`.

## Which snapshot writers should use this next?

Re-attributing the existing `live-unnest.zst` capture gives the following
shortlist. Decimal MB of outstanding allocations originating from each table's
snapshot upsert stack, including driver allocations. The first column is measured
at the **same** 628.80 MB process peak. The second is each writer's own maximum
and must not be summed. These are budgets to investigate, not predicted savings.

| Table | At process peak | Writer peak |
| --- | ---: | ---: |
| `passive_craft_state` | 43.0 MB | 52.4 MB |
| `vault_state_collectibles` | 39.2 MB | 46.0 MB |
| `experience_state` | 12.8 MB | 15.0 MB |
| `traveler_task_state` | 12.1 MB | 17.2 MB |
| `building_state` | 7.3 MB | 11.1 MB |
| `deployable_state` | 6.1 MB | 9.4 MB |

Attribution selects `websocket/initial_sync.rs:252–255` and the concrete caller,
or `vault_state/snapshot.rs:106–109` for vault's custom writer. It excludes
comparison queries, separate live writers and other custom workers; an absent
table isn't evidence of zero cost. The byte-capped `VALUES` capture independently
ranked passive crafts and vault first among these candidates. The local trace
directory above contains `table-audit.cpp` and both `table-audit-*.tsv` results.

**Proposed base:** keep the existing regional/global complete-snapshot reconciler.
Give its changed-row write step an optional table-specific writer that receives
owned models *before* conversion to `ActiveModel`; keep SeaORM `insert_many` as
the default. Share row/byte splitting and oversize handling with inventory.
For fixed-width scalar rows, derive a rows-per-statement limit from a conservative
encoded row size once; use a per-row estimate for variable payloads. Keep typed
arrays, conflict keys and updated columns explicit per table.

Start with passive crafts through this hook (single `entity_id`, nullable slot,
timestamp); retain the pre-snapshot flush. Then reuse the batching/writer contract
in vault, **not** the generic comparison loop: vault flattens collectibles,
checks duplicates and partitions oversized vaults. XP's cache observer must still
run on DB-unchanged rows. Keep stale deletion scoped and gated on successful
completion; partial location subscriptions already use fixed-shape upserts and
must not delete absent rows. For each migration, repeat the same-row replay
(insert, conflict update, failure before stale delete), peak heap, write time and
WAL comparison before switching more callers.

## Reproduce the final writer replay

Export JSONL with one `entity::inventory::Model` per line. For example, with
`psql -XqAt` on the source fixture, redirect the following SELECT to a private
file (no header). Use a separate disposable database for the replay.

```sql
SELECT row_to_json(sample) FROM (
  SELECT entity_id, pockets, inventory_index, cargo_index, owner_entity_id,
         player_owner_entity_id, region
  FROM inventory ORDER BY entity_id LIMIT 6000
) AS sample;
```

From `rust/api-server`, set `INVENTORY_WRITER_FIXTURE` to the JSONL path and
`INVENTORY_WRITER_TEST_URL` to the test PostgreSQL URL (permission to create
schemas). The test creates a private schema and drops it on success. Set
`INVENTORY_WRITER_TEST_BYTES` to override the default budget.

```sh
cargo test --locked -p api --features tokio-worker-queues,spacetimedb-tokio-websockets-rustls \
  --lib inventory::
cargo test --locked -p api --features tokio-worker-queues,spacetimedb-tokio-websockets-rustls \
  --lib inventory::snapshot_write::tests::replay_snapshot_writes_on_postgres -- --ignored --exact --nocapture
```

For heap measurements, build the test executable with `cargo test --no-run`
and run that executable (not Cargo) under `heaptrack --record-only` with
`--ignored --exact inventory::snapshot_write::tests::replay_snapshot_writes_on_postgres`.
