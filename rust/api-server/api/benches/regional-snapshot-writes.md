# Remaining large snapshot writers

This pass targets the five remaining shared-reconciler snapshot writers with
multi-megabyte peaks in the passive-craft/vault trace, plus two substantial
writers outside it: extraction recipes and the player-state DB queue.
These are **snapshot upserts**; live activity/history and changelog writes have
different ordering and durability requirements.

The cutoff for this pass was a measured writer-local peak of **5 MB**. For
player state the old queue joined live and snapshot writes, so its historical
stack cannot distinguish the two. This ranking is specific to these captures:
a rarely changed description table on a warm restart may cost more on a fresh
database.

| Table | Write contract |
| --- | --- |
| `traveler_task_state` | Keep the 6,000-row comparison limit and `entity_id` conflict key. |
| `experience_state` | Upsert on `(entity_id, skill_id)`; the write barrier and leaderboard observer (including unchanged DB rows) stay in place. |
| `building_state` | Persist claim, direction, description, constructor and region on conflict. |
| `deployable_state` | Insert the appearance override but **preserve the stored override on conflict**, just as before. Cap estimated bind payload at 1 MiB using UTF-8 nickname bytes; one oversized nickname travels alone. |
| `progressive_action_state` | Keep lock expiration as a serialized JSONB timestamp object, including microseconds. |
| `player_state` | Snapshot models travel through the same FIFO DB worker as prior live writes and region deletes. Preserve sign-in timestamp handling, presence/leaderboard updates and the transfer grace period. |
| `extraction_recipe_desc` | A byte-limited snapshot upsert, without treating absent recipes as deletions. Preserve nullable cargo (including zero), optional XP (`NULL` vs `[]`), and the existing JSON number representation for fractional probabilities/XP. |

All seven use one SQL shape per table, with typed array parameters and the
existing conflict keys. Fixed-width/bounded-shape tables retain their row cap.
Deployables and extraction recipes also use the same byte-splitting rule as
inventory. A byte budget bounds the estimated payload of a normal *statement*,
not the whole comparison batch or process. The two new variable-width writers
count oversized rows in `snapshot_oversize_rows_total{table=...}`.

## Isolated writer results

Peak is **whole test-process live heap in decimal MB** (one Heaptrack run
per variant). Insert/update times are medians of three alternating, unprofiled
runs, in ms, measuring only the writer call. WAL bytes are per-pass medians.

| Writer | Peak heap, before → after | Insert ms | Conflict update ms | Update WAL bytes, before → after |
| --- | ---: | ---: | ---: | ---: |
| Traveler tasks | 13.23 → 3.41 | 185 → 150 | 182 → 184 | 3,523,680 → 3,523,720 |
| XP | 4.06 → 1.28 | 225 → 184 | 225 → 217 | 3,593,056 → 3,593,032 |
| Buildings | 6.67 → 1.85 | 237 → 220 | 225 → 220 | 3,624,640 → 3,624,640 |
| Deployables | 14.58 → 7.09 | 251 → 254 | 263 → 287 | 3,920,168 → 3,920,104 |
| Progressive actions | 16.79 → 7.90 | 324 → 264 | 300 → 269 | 4,774,336 → 4,774,336 |
| Player state | 19.32 → 11.02 | 370 → 314 | 391 → 290 | 4,638,072 → 4,638,072 |
| Extraction recipes | 25.41 → 3.63 | 437 → 323 | 401 → 320 | 5,512,584 → 5,284,616 |

All seven replays produced matching database digests for inserts **and**
updates. This includes non-exact `f32` recipe probabilities, optional JSON,
microseconds and an oversized deployable with a multibyte nickname. The
deployable replay also sends a *different* appearance override on conflict
and verifies the original persists. Smaller batches cut peak memory but need
not improve throughput: deployable updates took ~9% longer in this run.
A mock failure on the second deployable subwrite checked that the third row
and stale-row deletion were not attempted.

## Whole-server Heaptrack comparison

Three serialized four-minute release runs with the system allocator, one global
and nine regional upstream subscriptions, fresh clones of the same claim-sorted
DB, and the same SDK revision. The baseline already includes compact claim
locations and the inventory, passive-craft and vault snapshot optimizations.
Concurrent SDK development was excluded from the compared binaries.

| Metric | Before | Five regional writers | All seven |
| --- | ---: | ---: | ---: |
| Whole-process peak live heap | 516.54 MB | 508.08 MB | **469.58 MB** |
| Median live heap, 200–235 s | 186.82 MB | 237.85 MB | 182.96 MB |
| Peak RSS, including Heaptrack overhead | 1.58 GB | 1.55 GB | 1.42 GB |

| Observed writer-local heap peak | Before | All seven |
| --- | ---: | ---: |
| Traveler tasks | 17.51 MB | 1.50 MB |
| XP | 15.46 MB | 0.74 MB |
| Buildings | 12.12 MB | 1.18 MB |
| Deployables | 9.52 MB | 1.19 MB |
| Progressive actions | 5.68 MB | 2.29 MB |
| Player state (includes the live upsert path for comparability) | 8.05 MB | 3.88 MB |
| Extraction recipes | 8.66 MB | 1.32 MB |

These are allocations attributed to the writer stacks, even if the DB driver
or pool subsequently retains them. Don't sum their separate maxima. At the
*actual global peak* they accounted for **47.85 MB together before and
0.28 MB after**. The observed whole-process peak fell **46.96 MB (9.1%)**.
The RSS comparison is instrumented; it is not a native allocator measurement.

Every run reached all ten connections, recovered a regional disconnect, read
20 inventory/vault completion markers, and drained gracefully. No migrated
writer logged an error. The intermediate run had two unrelated DB acquisition
timeouts. Live traffic is not deterministic: use the paired writer replays for
correctness, timing and WAL rather than interpreting differences between
successive server runs as exact marginal savings. The full API suite passed
with 399 tests (22 ignored); the seven PostgreSQL replays were run separately.
Local traces, per-stack attribution and replay logs are in the ignored directory
`rust/api-server/target/remaining-writers-profile-nqgkde/`.

## Reproducing the writer comparison

On a disposable PostgreSQL instance, set `SNAPSHOT_WRITER_TEST_URL` to a DB
where the replay can create a private schema. From `rust/api-server`:

```sh
cargo test --locked -p api --features tokio-worker-queues,spacetimedb-tokio-websockets-rustls \
  --lib websocket::snapshot_write::replay::regional -- --ignored --nocapture --test-threads=1
```

Repeat with `SNAPSHOT_WRITER_BASELINE=1` for the old `insert_many` path.
Each case inserts 12,019 rows and updates them on conflict, including a
partial last batch. It reads every column back after both passes and prints
a canonical database digest for comparison across variants. For Heaptrack,
run the test executable directly, selecting one ignored replay per process.
WAL is the per-pass change in `pg_current_wal_insert_lsn()` on an isolated
PostgreSQL 18 instance. Fixture tables are logged and have their primary keys;
they do not model every production index or competing transaction.
