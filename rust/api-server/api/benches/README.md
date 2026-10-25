# Ranking benchmarks

## Heaptrack allocation profiling

### Repeatable six-minute captures and graceful stop

Use `serve --shutdown-after-seconds 360` to request shutdown after six minutes,
or add this to your YAML configuration:

```yaml
shutdown:
  after_seconds: 360
  grace_seconds: 60
```

The duration starts on entering server startup (after the initial database
connection is opened), including migrations/cache loading, not after subscription
completion. The default `after_seconds: null` disables automatic stopping; signal
handling remains enabled. `0` requests immediate shutdown. CLI configuration
uses the existing environment/file precedence. An equivalent TOML table is
`[shutdown]` with `after_seconds = 360` and `grace_seconds = 60`.

Run the symbol-enabled system-allocator binary built below:

```sh
heaptrack --record-only -o /tmp/opencode/heaptrack-sync-6m target/heaptrack/release/bitcraft-hub-api serve --shutdown-after-seconds 360
```

Timer expiry, Ctrl+C and SIGTERM share one shutdown path. Axum stops accepting
requests and lets in-flight requests finish; upgraded WebSocket clients receive
a best-effort close frame with a bounded wait. SpacetimeDB connections disconnect
and are driven to apply the queued disconnect; startup/retry sleeps are cancelled
and no reconnect is attempted. After producers stop, worker inputs reject new
messages but drain accepted messages, even when a worker retains its own sender.
Kanal's destructive channel close is deliberately not used. Final batches and
database-write queues are awaited, including bounded best-effort claim events.
The ranking publisher processes pending scores and joins, then state/pool cleanup
finishes. Snapshot completion/stale-deletion checks are unchanged.

Draining is additional to the run duration, so a six-minute capture can take up
to approximately seven minutes with the default grace period. If the grace
period expires, unfinished async work is cancelled with an error log/nonzero
exit; queued writes may be lost, so don't treat that run as a completed sync.
Cancellation cannot preempt synchronous SDK callbacks or blocking code. Existing
database error handling/best-effort event retry limits are unchanged; graceful
shutdown is not a new durability guarantee.

`startup_memory` stderr snapshots mark `shutdown_requested`,
`shutdown_drained_state_retained`, and `shutdown_state_dropped`. Compare the same
duration, regions, queue backend and chunk settings, and verify **Graceful
shutdown complete** in logs. A fixed duration does not ensure equivalent sync
progress under different profiling overhead. Keep captures under distinct names
when comparing changes. No production server is started/stopped by the tests.

### Profiling build

Normal server builds use mimalloc, which bypasses Heaptrack's normal malloc/free
hooks. Enable the root package's `system-allocator` feature for captures; without
the custom global allocator Rust uses the system allocator. Normal builds remain
unchanged. Compare allocation sites with this build, not absolute RSS against a
mimalloc run.

Build a separate profiling binary (preserving the normal release binary):

```sh
CARGO_TARGET_DIR=target/heaptrack CARGO_PROFILE_RELEASE_DEBUG=2 cargo build --release -p bitcraft-hub-api --features system-allocator --jobs 4
```

The debug override supplies full symbols for dependencies too. Do not strip the
binary; keep it and its source checkout unchanged while analyzing the capture.
The current temporary release optimization settings still apply.

Start with database initialization only, against an already migrated database:

```sh
heaptrack --record-only -o /tmp/opencode/heaptrack-init target/heaptrack/release/bitcraft-hub-api --database-url 'postgres://postgres:password@localhost/ea3' profile-init --settle-seconds 5
```

For full synchronization, stop the existing server first to avoid port conflicts
and duplicate synchronization workers. Set `spacetimedb.skip_state_tables` to
`false`, keep regions/chunk settings consistent, and run with your normal global
configuration arguments before `serve`:

```sh
heaptrack --record-only -o /tmp/opencode/heaptrack-sync target/heaptrack/release/bitcraft-hub-api serve
```

Allow initial synchronization to complete and settle, then stop with Ctrl+C.
Use `python3 scripts/worker_metrics.py --rss --samples 3 --interval 5` from another
terminal to observe progress without dumping the metrics endpoint. Heaptrack can
add significant runtime and disk overhead; check available space for full-sync
captures. Startup capture is preferable to attaching to the running process.

Heaptrack prints the actual output filename (including compression suffix).
Open that file with `heaptrack_gui <file>` or inspect it with
`heaptrack_print <file>`. Inspect peak/live allocations and their call stacks,
then allocation counts/temporary allocations for churn. Use the GUI timeline to
compare the initialization peak with the settled period; end-of-process leak
totals alone do not describe steady-state usage. Heaptrack tracks requested heap
bytes, not every source of RSS or allocator-retained pages.

## Compact claim-member indexes

The full-sync Heaptrack capture reported large allocation-site peaks for the
inner claim-member `DashMap`s (~120 MB) and player-to-claim `DashSet`s (~180 MB).
These are site peaks, not necessarily simultaneous usage or predicted savings.
The indexes now keep sharding only in the outer maps. Inner values are ordinary
`HashMap<membership_id, Model>` and `HashSet<claim_id>`, protected by the outer
entry guards. This removes a shard/lock array per claim/player without changing
the membership payloads or API responses. Readers copy response data before
releasing the guard, with no guard held across an await.

Entry-based insertion also avoids overwriting concurrent first inserts. Removal
now targets the relevant claim, uses the player ID for the reverse index, and
drops empty inner collections/outer entries. Tests cover replacement, duplicate
membership delivery, other claims/players, empty-entry cleanup, and concurrent
first insertion. Updates to the two indexes remain separate, as before; this is
not a transaction across both maps. Snapshot/database reconciliation and worker
ordering are unchanged. Rebuild/restart and repeat the same full-sync capture to
measure RSS and allocation changes; no whole-process saving is assumed yet.

## Inventory snapshot JSON decoding

Inventory snapshot comparison now uses `inventory::db_row::InventoryDbRow`, a
read-only projection with a transparent typed `Vec<Pocket>` JSON wrapper.
SeaORM's normal `Vec<Pocket>` extraction first decodes a generic
`serde_json::Value` tree (allocating maps, key strings and enum strings), then
converts each element. The wrapper selects SeaORM/SQLx's direct `Json<Self>`
decoder on PostgreSQL instead. Only the existing-row snapshot comparison query
uses this projection; entity types, other inventory reads, JSON storage/writes,
selected SQL columns, regional filters, batch sizes and reconciliation ordering
are unchanged. Decode failures still abort reconciliation before stale deletion.

Two ignored allocation workloads in `api/src/inventory/db_row.rs` each decode
10,000 arrays of 96 pockets (empty slots, item/cargo stacks, optional durability),
asserting equal results. The legacy workload calls SeaORM's actual
`TryGetableFromJson::from_json_vec` after generic JSON parsing. A local Heaptrack
run with the optimized test profile/system allocator measured:

| Decoder | Allocation calls (including test harness) |
| --- | ---: |
| Generic JSON tree then typed pockets | 7,800,699 |
| Direct typed pockets | 60,699 |

This is ~99.2% fewer allocation calls for this synthetic decoder workload, not a
whole-server RSS/throughput claim. Captures are
`/tmp/opencode/inventory-json-{legacy,direct}-exact.zst`. To reproduce, build with
`cargo test -p api --lib --no-run --jobs 4`, then run the printed test executable
under Heaptrack with one of these exact test names and `--ignored --exact`:

- `inventory::db_row::tests::profile_legacy_json_decoding`
- `inventory::db_row::tests::profile_direct_json_decoding`

Unit tests check empty/populated arrays, JSON roundtrips, ignored unknown fields
and malformed data. The optional PostgreSQL test compares both row decoders on
read-only SELECT fixtures, including SQL NULL and missing columns:

```sh
INVENTORY_TEST_DATABASE_URL='postgres://postgres:password@localhost/ea3' cargo test -p api --lib inventory::db_row::tests::postgres_direct_decode_matches_entity_and_propagates_errors -- --ignored --exact
```

The PostgreSQL test passed locally. Full-sync results require a rebuilt server
and equivalent synchronization configuration/progress.

## Owned initial-snapshot delivery

The API's listener macro now registers `on_initial_owned`, receiving `Vec<Row>`
instead of a borrowed `InitialRows` view. The local SDK and Rust generator add a
second initial-dispatch phase: first invoke borrowed/legacy callbacks, drop the
applied diff (which borrows decoded rows), then dispatch owned initial rows from
the mutable update. Cacheless single-owned-listener dispatch moves row payloads
out of the SDK, including nested strings and vectors; it does not deep-clone the
snapshot. No initial `on_insert` callbacks are emitted for tables with initial
listeners, as before. Registration persists across snapshots/reconnects.

Borrowed and legacy APIs remain supported. Extra owned listeners receive clones
so every listener still gets a complete snapshot; which listener receives the
original is unspecified. Client-cache builds copy typed rows instead of draining
the parsed inserts. Older generated bindings remain compatible with existing
callbacks, but owned delivery requires the new generated DbUpdate dispatch hook.
The API's checked-in bindings include that hook for all 327 tables, with versioned
table-name mappings preserved, and generic registration/removal on RemoteTables.

Application handoff now moves the full vector directly when chunking is disabled;
inventory/vault chunking moves rows into chunks without cloning payloads. The
queues remain unbounded. Snapshot completion, interrupted-snapshot safeguards,
and ordering against later live updates remain unchanged. Row-array buffers may
still be allocated/repacked, and decoded snapshots still materialize in full;
this removes duplicate payloads, not all snapshot or queue memory.

Tests cover original payload pointers and clone counts, mixed and multiple
listeners, cache-enabled copies, repeated/empty snapshots, live-update exclusion,
removal, and non-Clone worker payloads/full-vector allocation transfer. Production
RSS impact remains pending a rebuild/restart and equivalent Heaptrack capture.

## Database initialization RSS diagnostic

From `rust/api-server`, run against an already migrated database:

```sh
cargo run --release -- --database-url 'postgres://postgres:password@localhost/ea3' profile-init --settle-seconds 5
```

This loads the same caches and rankings as server startup, then exits without
running migrations, HTTP, SpacetimeDB, or the broadcaster. Normal `serve` is
unchanged. Configuration and connection-pool settings are the same as `serve`;
the pool itself contributes to RSS (the default minimum is 30 connections).

`startup_memory` lines go to stderr even if tracing is filtered. Linux
`/proc/self/status` supplies current RSS and lifetime peak RSS in KiB; peak RSS
also captures transient peaks between snapshots. Other platforms report memory
as unavailable. Snapshots cover connection setup, empty state, parallel cache
loads, opening the experience stream, finishing/dropping it, and settling.
Cache counts, consumed experience row counts, and final ranking buffer counts help
separate live payloads from total RSS. They do not measure allocator overhead or
all live allocations. Final ranking counts exclude the worker's in-flight batch.

The settling delay retains the app state; it neither forces allocator collection
nor guarantees ranking publication has completed. Increase it if necessary.
All ten startup queries now stream rows. Cache rows are moved directly into
their maps, and experience remains ordered by player/skill for running totals.
Query/stream errors abort initialization rather than serving partial state.
This command measures initialization, not subsequent live synchronization.

An initial before/after run using the temporary `opt-level = 1`, `lto = false`
release settings and the same database (970,180 experience rows, 48,509 players):

| Stage | Full-result loads | Streaming loads |
| --- | ---: | ---: |
| Caches loaded RSS | 93 MiB | 89 MiB |
| Final/peak RSS | 302 MiB | 148 MiB |
| Initialization including connection setup | 1.13 s | 1.09 s |

Both runs had identical cache and experience row counts and no final pending
ranking changes. This is a single-run comparison, not a production guarantee;
allocator retention and worker scheduling vary. Raw logs are in
`/tmp/opencode/profile-init-opt1.log` and
`/tmp/opencode/profile-init-streaming.log`. There is no longer a full experience
result vector (previously 72 MiB capacity).

Startup now counts `player_state` before ingestion and uses that count as a
ranking capacity hint. The shared ID map/vector, immediate/published score and
presence arrays, ranks, sorted order and merge scratch are reserved without
creating members or changing lengths. Skill boards created later inherit the
hint, and all structures can still grow beyond it (for example, experience-only
players or concurrent database changes).

Temporary pending-map reservations are capped at 16,384 entries per board and
the changed-player scratch at 8,192, matching the existing bounded-retention
policy instead of permanently reserving full-player batches. HashMap capacity
can round up. Bucket BTreeMaps have no reservation API and remain demand-grown.
This is a global player-count hint, so sparsely populated boards can reserve more
than needed; it is not a per-board membership estimate.

With the same database and iteration settings, the first hinted run measured
126 MiB final/peak RSS versus 148 MiB streaming-only, and 26.8 MiB ranking array
capacity versus 38.7 MiB. Initialization including connection setup was 1.23 s
versus 1.09 s; these are single runs, not evidence of a latency improvement.
Raw log: `/tmp/opencode/profile-init-capacity-hint.log`.

### Streaming reconciliation keys

The shared `websocket::initial_sync` reconciler streams database primary-key
tuples directly into the stale-key `HashSet` instead of fetching all query rows
and then collecting their decoded keys. The scoped stream is dropped before any
comparison/write query, so it does not retain a pool connection during batching.
Regional/global filters, composite-key matching and complete-snapshot deletion
rules are unchanged. Opening, reading or decoding failures abort before consuming
the incoming snapshot or issuing stale deletes, including failures after some
keys have already been read.

This targets the ~75.5 MB PostgreSQL result-vector site in the six-minute
Heaptrack capture. It does not remove the retained key set, driver per-row
allocations, or full decoded SDK snapshots. The set can still grow/rehash during
streaming; actual whole-process peak/RSS savings require a rebuilt comparison.

### Reconciliation scratch buffers and composite filters

The shared reconciler reuses incoming-row, existing-key and changed-active-model
vectors plus its existing-row lookup map across comparison batches. Rows are moved
with `drain(..)`; the map is cleared before awaiting writes so only its backing
allocation survives. Scratch storage is local to one snapshot and grows only with
the configured comparison batch size, not the snapshot length. Full-row comparison
queries now stream directly into the lookup map, avoiding intermediate result/model
vectors. The stream is dropped before writes; partial/decode failures abort before
observing the batch or deleting stale rows. SQL construction still allocates per batch.

Composite-key filters sort one owned key copy and build exact per-entity groups
from contiguous slices, replacing the grouping hash table and its per-entity
vectors. Caller keys are unchanged; each entity condition still pairs its ID with
only its own component IDs. SQL expression/value allocations remain. Sorting adds
O(n log n) work, so latency as well as allocations should be compared.

These changes have not yet been measured in a rebuilt equivalent six-minute
Heaptrack run; no peak-heap or allocation-count savings are claimed. Incoming,
comparison and write failures still prevent stale deletion, and a partial incoming
batch is not observed or written after a stream error.

### Housing-related query/write overlap

The five housing helpers (interior networks, dimensions, player housing,
permissions and portals) use `exec_without_returning()` because their callers
discard generated primary keys. Zero affected rows still produce
`RecordNotInserted`, preserving the old retry behavior. Original ActiveModels
remain untouched until the helper succeeds; no retry payload is blanket-drained.

Writes are split using an advisory 256 KiB estimated per-statement budget:
ActiveModel size plus 128 bytes per column for SQL/bind overhead, and a conservative
JSON-length estimate for interior-network child IDs. The estimate is not a hard
encoded-byte or process-memory limit; oversized single rows remain supported.
Split writes run sequentially in one transaction, preserving all-or-nothing
behavior and retaining the entire retry batch on failure. A single-statement batch
does not add an explicit transaction. This limits simultaneous query construction,
not the memory of originals kept for retry, and may add round-trip/transaction
overhead. Heaptrack/Samply comparisons are needed before claiming savings.

### Inventory reconciliation scratch buffers

The separate inventory reconciler now streams existing IDs directly into its
stale-ID set. Typed JSON comparison rows stream directly into a reusable lookup
map instead of passing through full-result/model vectors. Both streams are scoped
and dropped before the next comparison or write, returning their pool connection.
Incoming rows, comparison keys, existing-row lookup capacity and changed-model
storage are reused within one snapshot; existing-row payloads are cleared before writes,
and inserts consume changed models with `drain(..)` to retain vector capacity.

Partial incoming/key/comparison failures abort before stale deletion. Snapshot
upsert failures now also abort reconciliation rather than merely logging and
continuing to stale deletion; live-update write behavior is unchanged. Region
filters, typed pocket JSON decoding, bounded batches and chunk completion remain
unchanged. This does not bound snapshot queues or remove the large decoded-pocket
backlog. Whole-process CPU, allocation-count and peak-memory savings remain
unmeasured until an equivalent rebuilt profiling run.

### Sparse generated SDK updates

The local Rust generator now stores `DbUpdate` and `AppliedDiff` table payloads
in sparse vectors of typed enum variants rather than one field per schema table.
Default wrappers are each one vector (24 bytes on x86-64), with zero heap capacity;
nonempty updates allocate aggregate vectors for touched tables, not per-table boxes.
This removes construction/drop and dispatch work for absent tables. It does not
remove payload ownership or the cost of processing tables actually present.

Repeated chunks for a table are merged in arrival order. Cache application still
uses tables-then-views order; borrowed and owned callbacks retain accessor-name
order. Present empty initial snapshots remain distinguishable from absent tables.
The generator's opt-in compiled-fixture test covers both SDK cache modes and
also compiles bindings for a module with no tables.

These changes require regenerating Rust bindings with the updated local generator
and rebuilding before profiling. The new aggregate-vector allocations are a CPU
versus allocation tradeoff; whole-process improvements have not yet been measured.

### SDK decompression experiment (removed)

Connection-local adaptive decompression scratch space was tested and removed:
full-sync captures did not establish an overall memory improvement, and a roughly
68 MB Brotli output-vector growth site remained. The SDK again uses fresh
per-message decompression vectors; uncompressed messages still borrow the input.
The streaming owned-BSATN decoding idea is saved for later in the SDK repository
at `sdks/rust/STREAMING_DECOMPRESSION.md`.

### Lazy write-batch cloning

The 26 active `insert_many(messages.clone())` helpers now pass
`messages.iter().cloned()`. SeaORM consumes each cloned ActiveModel as it builds
the query, avoiding an eager full-batch ActiveModel vector while retaining the
original rows across the database await. Field payloads and SQL values still
require copying; this is not a zero-copy write path, and most SQL-driver
allocations are unaffected.

The five housing helpers still clear only on success, retaining the exact batch
and its vector capacity on failure for retry. Other audited helpers already log
errors and clear even on failure; this change preserves that existing policy
rather than introducing retries or changing cancellation behavior. No helper was
blanket-converted to `drain(..)`. Whole-process savings require a rebuilt,
equivalent Heaptrack capture; the ~15.4 MB housing write-site peak includes SQL
construction/driver allocations and is not a promised saving.

### Description synchronization allocation reductions

**Temporary state-subscription isolation:** `spacetimedb.skip_state_tables` now
defaults to `true` for RSS testing. The current temporary diagnostic uses
**zero subscriptions**, including on later regional connections. No subscription
request is sent; connections and workers still run. All description/state tables
and raw SQL joins are disabled before download or SDK caching. Database startup
loading is unchanged. Tables and claim events do not receive live updates in
this mode. Restart the process for each
RSS comparison: changing subscriptions
does not reclaim previous process allocations or clear persisted data.

Set `skip_state_tables: false` under `spacetimedb` in the YAML config to restore
normal sync, or use the equivalent TOML/JSON setting. A warning is emitted for
every regional connection while the diagnostic is active. Restore the default
to `false` when this experiment is finished. Background workers are still
created, so this isolates subscriptions/data rather than all sync infrastructure.

Item, cargo, item-list, crafting-recipe, skill, traveler-task, NPC, building, and
collectible description snapshots now use bounded reconciliation instead of
retaining a full database model map. The helper retains all primary keys for
stale-row detection, but fetches full models only for one incoming batch at a
time; new IDs skip full-row reads and unchanged rows skip writes. The incoming
SDK snapshot allocation itself is unchanged.

Cached descriptions compare the incoming value against the current cache entry
before cloning/replacing it, including live inserts/updates. Cache comparison is
independent of database comparison, so a missing/stale cache is still repaired
when the database row already matches. Collectibles remain uncached as before.
Buffered older live writes/deletes are flushed before a snapshot; reconciliation
errors abort before stale-row deletion (earlier successful batches may remain).

These changes affect live SpacetimeDB synchronization, not `profile-init`.
Their whole-process RSS impact has not yet been measured under live sync.

### Idle worker allocation changes

Legacy batch workers now keep initially empty write/delete buffers outside the
batch loop, reusing their capacity after flushing instead of reserving thousands
of rows on every idle timer tick. Stateful worker constructors and the mobile
receive buffer also start without reserved row capacity. Buffers handed off to
separate database-worker queues still transfer ownership as before.

The shared `BatchTimer` is unarmed while idle. The first received message starts
the batch deadline; subsequent messages do not extend it. Size limits still flush
immediately. Channel closure flushes partial batches without waiting for the
timer and drains queued messages before exit. These changes do not alter the
current zero-subscription diagnostic or skip worker creation; rerun that same
configuration to isolate their RSS impact. No production RSS saving is assumed
until measured.

Queue handoffs in player, username, experience, inventory/changelog, location,
and claim workers now use `mem::take` rather than replacing a sent batch with an
immediately preallocated full-size Vec. The existing batch allocation moves into
the database queue unchanged; the producer has zero capacity until more rows
arrive. This avoids reserving a second buffer after a final batch, but is not a
recycling pool: queued buffers remain allocated until consumed, and later batches
allocate on demand. Explicit player/username snapshot refill reservations were
also removed. Measure with full sync enabled to compare against the previous
~800 MB observation; no RSS reduction has yet been measured for this change.

### Inventory/vault initial-snapshot chunking experiment

These are the first two active targets based on database size (vault collectibles
~1 GiB, inventory ~414 MiB on disk); disk size is not an RSS measurement.
Location is larger but is not currently subscribed.

```yaml
spacetimedb:
  skip_state_tables: false
  initial_batch_rows: 1000  # 0 restores the original one-vector snapshot copy
```

Only inventory and vault callbacks use this setting. Instead of `new.to_vec()`
for the complete SDK snapshot, each clones at most this many **SDK rows** into a
separate vector. A vault row can contain many collectibles, so this is not a
byte limit or a bound on flattened database rows. The existing database batch
sizes remain unchanged. The worker receives one snapshot envelope with a private
chunk channel, consumes chunks in order, and drops each exhausted vector. Older
snapshots/live updates for that worker still precede newer envelopes. The explicit
completion marker must be read before stale-row deletion; interrupted streams
abort reconciliation without deleting unseen rows.

This deliberately keeps chunk queues unbounded to isolate allocation-size/reuse
effects. All chunks can still accumulate, and the SDK still materializes the
original full snapshot slice. Neither lower peak RSS nor lower settled RSS is
guaranteed. `scripts/worker_metrics.py` reports the initial-batch subqueues to
help interpret the experiment. Restart for each A/B run and compare the same
regions and settled sync state. Live RSS results for this change are pending.

### Worker input queue backend comparison

SDK-to-worker input queues now default to **Kanal unbounded**. Database-write
queues (bounded and unbounded), outbound WebSocket queues, and private initial
snapshot chunk queues are unchanged to isolate the input-channel experiment.
Subscriptions, batch limits, and snapshot chunk size also stay unchanged.

The adapter preserves FIFO, sender-drop/drain behavior, and receive batching.
Kanal 0.1.1 explicitly warns against cancelling its async receive futures in
`tokio::select!`. A persistent owned stream retains the in-progress receive when
the worker's batch timer wins. It is pinned once per channel, not boxed anew per
message. An atomic outstanding count includes values handed directly to that
pending receive, preventing early worker exit on sender closure. Tests cover
repeated cancellation with a concurrent producer, FIFO and drain behavior.

Use the same config/regions/chunk settings for both release builds:

```sh
# Kanal (default)
cargo run --release -- serve
# Original Tokio input queues
cargo run --release --features api/tokio-worker-queues -- serve
```

Add your usual database/config arguments. Startup logs identify the selected
backend. Restart between runs, wait for equivalent synchronization progress, and
use `python3 scripts/worker_metrics.py --rss --samples 3 --interval 5`.
Kanal RSS results are pending; changing libraries does not change queued payload
sizes or guarantee lower RSS.

### Removing the SDK's hidden initial-row clone

The API now uses the local SDK at
`/home/resubaka/code/SpacetimeDB/sdks/rust` (`default-features = false`) instead of
the pinned Git dependency, so local SDK changes actually participate in builds.
The SDK and Rust generator add an `InitialRows<'_, Row>` borrowed-view callback
API. The generated `RemoteTables` entry point is wired into all API snapshot
listeners without regenerating every individual table handle.

Before this change, SDK callback dispatch deep-cloned every decoded row into a
full-table `Vec<Row>`, then application dispatch cloned those rows again into
worker-owned vectors/chunks. Borrowed callbacks now read the SDK's decoded rows
directly; only the application's ownership handoff clones remain. Inventory and
vault still use the configured application chunk size; other workers still
receive their existing full-vector envelope. Network decoding, serialized-byte
storage, worker queue backend and reconciliation semantics are unchanged.

The existing `on_initial(&[Row])` API remains supported. If a table has legacy
callbacks, they share one cloned vector as before; borrowed-only registration
performs zero SDK row clones. Tests check clone counts, original-row identity,
mixed/removed legacy callbacks, empty snapshots and borrowed chunks. Snapshot
callback lifetime prevents retaining borrowed rows after dispatch returns.

Validation: SDK library tests with and without client cache, API non-binding-
export tests, and code-generator compilation. Production RSS results are pending;
restart/rebuild the API with the same regions and chunk settings for comparison.

The local SDK's `TableUpdate::append` also adopts the first nonempty parsed
insert/delete vector instead of allocating another backing array to append into
an empty destination. Subsequent chunks still append in order; empty inputs and
initial/present flags keep their previous semantics. Pointer/capacity tests use
real parsed row lists to verify the allocation transfers unchanged, and drop
tests verify payload ownership. This removes the extra row-array allocation,
not the wire buffers or decoded row payloads. RSS impact remains to be measured.

The cacheless SDK parser now drops each serialized `Bytes` row slice after
decoding instead of retaining it in `WithBsatn` alongside the owned row. This
lets packed row-list backing buffers be released after parsing rather than
remaining pinned by typed updates/SDK queues. The public `bsatn` field remains
present but is empty in this mode, so its per-row struct overhead is unchanged.
Client-cache builds always keep the bytes needed for hashing/refcounts. The
optional SDK feature `retain-row-bytes` restores raw-byte retention for cacheless
consumers that inspect the field; the API does not enable it.

Ownership tests track destruction of the actual input buffer while decoded rows
are still alive. They cover initial rows, deletes, persistent/event updates,
decode failures and owned strings/vectors. Parsed primary-key matching is also
tested without raw row bytes. Decoder/network buffers that are still in use and
allocator-retained pages are not forcibly reclaimed. Production RSS results for
this change remain pending a rebuild/restart.

Run the current implementation in release mode:

```sh
cargo bench -p api --features bench --bench leaderboard_get_rank
```

Coverage includes rank lookups at 10,000, 48,506 and 100,000 players; score
lookups; top-100 and deep pagination; publishing batches of 1–5,000 changed
players; and initial population.

The harness publishes the initial ranking before measuring reads. Update timings
include both ingestion and synchronous index publication, rather than just
enqueueing work. They exclude the live worker's 20 ms refresh pacing, OS thread
scheduling, database traffic and contention between concurrent requests.

The dense implementation uses sparse repositioning for up to 128 changed players
and a sorted batch merge for larger batches. Small score changes and large jumps
can have different costs: cached rank positions must be refreshed for every
player crossed. These microbenchmarks are not a replacement for a production
load test.

The development comparison against the tree-based implementation from commit
`f4d9cd8b` is isolated under `/tmp/opencode/ranking-performance`. That comparison
compiles the actual working-tree compact implementation alongside the extracted
tree implementation, using the same release optimizations and mimalloc allocator.
It uses positive scores to avoid the old implementation's zero-score bookkeeping
bug. The tree baseline predates the coalescing changes as well as compact storage,
so repeated-player update results include the benefit of coalescing.

## Development comparison

Measured on an AMD Ryzen 9 9950X3D with Rust 1.99.1, `opt-level=3`, LTO and
mimalloc, using 30 Criterion samples per case and 48,506 players. Rounded central
estimates from the final sparse-path run:

| Operation | Tree baseline | Compact arrays |
| --- | ---: | ---: |
| Rank lookup, p10 player | 351 ns | 38 ns |
| Rank lookup, p50 player | 1,252 ns | 38 ns |
| Rank lookup, p90 player | 410 ns | 38 ns |
| Immediate score lookup | 18.4 ns | 28.1 ns |
| Top 100 | 346 ns | 152 ns |
| 100 entries after offset 24,000 | 59.6 µs | 0.153 µs |
| Update and publish 1 player | 0.530 µs | 0.234 µs |
| Update and publish 10 players | 1.86 µs | 2.14 µs |
| Update and publish 100 players | 16.2 µs | 23.8 µs |
| Update and publish 1,000 players | 444 µs | 301 µs |
| Update and publish 5,000 players | 2.32 ms | 0.856 ms |
| 1,000 successive updates to one player, then publish | 190 µs | 43.5 µs |
| Construct, populate, publish, validate and drop board | 8.85 ms | 9.79 ms |

The first compact implementation merged the entire array even for one changed
player, taking about 161 µs in that case. The sparse path fixes that regression.
It does not make every operation faster: medium-sized batches, score lookup and
initial construction remain slower in this workload. Large jumps across the
ranking can also cost more than small score changes because crossed players'
cached ranks must be refreshed. This is a memory/read-performance trade-off, not
a guarantee of lower CPU use for every production workload.

These publication timings include ingestion, synchronous index work and one rank
lookup; the 20 ms live refresh pacing is excluded. Raw results are under
`/tmp/opencode/ranking-performance/target/criterion` (the final run uses the
`sparse` baseline name). Differences in hardware, score distributions and reader/
writer contention can change these results.

## RSS regression investigation

The original array-payload test was **not** a whole-process memory comparison.
An instrumented release probe subsequently reproduced higher RSS despite lower
live allocation sizes. The probe uses separate processes, mimalloc, 25 boards
(18 narrow/7 wide), 48,506 players and live asynchronous publication:

| Dense-board scenario | Live requested heap | Settled process RSS |
| --- | ---: | ---: |
| Tree baseline, 25 workers | ~107 MiB | ~151 MiB |
| Initial compact rewrite, 25 workers | ~36 MiB | ~375 MiB |
| Shared publisher and reusable buffers, three runs | ~49–52 MiB | ~95–117 MiB |

The fix uses one publisher per `RankingSystem`, weak board registrations, and
reuses normal pending/merge buffers instead of repeatedly allocating and freeing
them across many producer/worker heaps. Oversized pending maps are not retained
indefinitely. Those measurements used a 20 ms cadence with coalescing; all boards
are serviced by one worker, so simultaneous heavy publications can affect refresh latency.
The earlier CPU microbenchmarks do not measure that contention/latency trade-off.

These figures are diagnostic, synthetic ranking-only measurements—not production
RSS predictions. The allocation counter measures requested live layouts, not
allocator usable sizes, and adds allocation overhead that can affect scheduling.
RSS includes allocator-retained pages, stacks and mappings. Raw logs and the
counting-allocator probe are under `/tmp/opencode/ranking-performance`
(`memory.rs`, `memory-comparison.txt`).

Dense storage also has a real sparse-board trade-off: one full board plus 24
boards containing only 1% of registered players measured ~31 MiB RSS for trees
versus ~51 MiB for the shared-worker compact version. Stable shared indices leave
holes and do not reclaim historic player slots. Do not assume compact storage is
always smaller when board membership is sparse.

The `/metrics` endpoint now exposes:

- `leaderboard_registered_players`: shared registry size, including historic IDs.
- `leaderboard_active_players{leaderboard=...}`: current members of each board.
- `leaderboard_score_slots{leaderboard=...}`: logical dense slots in each board.
- `leaderboard_buffer_capacity_bytes{leaderboard=...}`: all score, presence,
  sorted-player, rank and reusable-work Vec payload capacities (not total heap/RSS).
- `leaderboard_pending_changes{leaderboard=...}` and
  `leaderboard_pending_change_capacity{leaderboard=...}`: pending-map length and
  capacity. Map allocator overhead and the publisher's in-flight buffer are not
  included in the buffer-byte gauge.

Compare active membership with slot counts to identify boards that would benefit
from sparse/paged storage, and compare total buffer bytes with RSS to distinguish
live ranking buffers from other allocations and allocator retention.

### Live ranking publication: 100 ms and unchanged-position fast path

The current publisher uses a 100 ms minimum interval between completed
publications instead of 20 ms. An 800 ms experiment increased large-batch merge
work; 100 ms is intended to keep more batches within the 128-player sparse-update
threshold, but does not guarantee it under load. The first publication after startup/idle can run
immediately. Immediate score updates and per-event WebSocket notifications are
unchanged, but rank fields still use the last published ranking and can be older.
This is pacing, not a hard latency bound: publishing and scheduling add time.
Shutdown still drains pending rankings and can wait for the remaining interval.

Sparse score updates first compare the adjacent player in the direction of
movement, using the existing score/player-ID tie order. If the rank stays the
same, only score/bucket bookkeeping changes: there is no binary search, array
rotation, or rank-position rewrite. Actual moves retain directional binary search;
insertions/deletions and the large-batch merge path are unchanged. Longer intervals
may shift more batches into that merge path, so CPU gains require a rebuilt live
capture; historical microbenchmarks above exclude refresh pacing.

### Experience worker: indexed pending writes and per-event calculations

Live persistence uses a reusable map of models and a set of deletes keyed by the
actual `(entity_id, skill_id)` primary key, not by region. Upserts cancel pending
deletes and replace earlier upserts; deletes cancel pending upserts and deduplicate
repeated removals. ActiveModels are built only when draining to the existing
bounded database-operation queue. Statement row order is unspecified, but per-key
last-operation semantics, upserts-before-deletes flush order, and snapshot barriers
are preserved. Error handling and the database queue capacity are unchanged.

Level conversion binary-searches the sorted XP thresholds, preserving the old
negative-XP fallback and exact-boundary results. Reusable per-event scratch stores
new levels/categories; unchanged old quantities reuse the new level, and matching
old skills reuse the category. Names/categories are never cached across events.
Changed-skill notifications share one description lookup, and board updates use
shared rather than exclusive skill-board map guards. Notifications remain per-event
and in the original order; they are not coalesced with persistence.

The audit also fixed XP-increase persistence queuing the old rather than new XP.
The pre-existing update policies for XP decreases and added/removed skills are
otherwise unchanged and remain a separate correctness audit, not an optimization.
Tests cover primary-key/region supersession, flush order, snapshot draining,
notification order, refreshed metadata, and all level thresholds. CPU/allocation
effects still need a rebuilt equivalent-workload profile; retained indexes trade
some worker memory for avoiding scans and repeated ActiveModel construction.

### Broadcast routing, shared inventory descriptions, and crafting batches

Broadcast topics now use a three-slot inline list. Fixed topic names are borrowed;
skill-specific formatted names remain owned. Topics are built once per message.
Fan-out scans matching clients directly under one read guard, without recipient
vectors or awaits inside that guard. Topic order, overlapping-subscription duplicate
deliveries, the 2,000-message in-flight bound, and slow-client disconnect behavior
are preserved. Keyed subscriptions cannot match a global (`None`) topic. Socket
writers encode borrowed messages rather than deep-cloning before serialization.
Per-recipient owned messages remain, avoiding a broad channel/protocol redesign.

Inventory expanded descriptions are immutable `Arc<Model>` snapshots with unchanged
JSON, MessagePack, and TypeScript shapes. The inventory worker caches at most 256
item and 256 cargo descriptions; cache pressure clears the corresponding map.
Each lookup reads and compares the current source model before reusing an Arc.
Changes replace the snapshot, removals fail resolution, and queued notifications
retain their original descriptions. This works with all description writers without
an invalidation side channel. It trades structural comparisons and bounded extra
retention for fewer description/string clones across pockets, events, and recipients;
HTTP resolution remains uncached. Missing descriptions still suppress the entire
sync notification, never raw persistence.

Progressive-action persistence now coalesces live upserts/deletes by entity primary
key, builds ActiveModels at flush time, and reuses batch allocations. It emits no
notifications itself. Live failures retain the existing logged best-effort policy.
Before an initial snapshot, preceding live writes are flushed; a failure aborts
snapshot processing. Snapshots use the shared streamed reconciler so key/comparison
or upsert failures cannot proceed to stale deletion. Per-key last-operation semantics
and upserts-before-deletes flush order remain; row order within statements is not
specified. Tests cover routing/backpressure, wire-shape compatibility, snapshot
refresh/retention bounds, coalescing, snapshot errors, and shutdown draining.
CPU/allocation savings require a rebuilt equivalent-workload capture.

### Database metric label parsing

The statement callback reads only the first token for the method instead of
collecting four tokens. Table extraction uses a single iterator pass without a
token vector, returning a borrowed slice. It preserves the original heuristic's
first-match behavior, `FROM > INTO > UPDATE > TABLE` precedence, trimming, and
fallback labels; it is not a new SQL parser. Method recognition still accepts
only the original all-uppercase/all-lowercase spellings, independently of the
case-insensitive table-keyword matching.

Method/failure/unknown labels borrow static storage. Runtime table names are
owned once so metric keys remain valid after the statement callback. Counter and
histogram names, labels, counts, and recorded duration units are unchanged; this
does not add a statement/metric registry cache. Compatibility tests compare the
new extractor against the former implementation over fixtures and generated
keyword combinations. CPU/allocation impact still needs rebuilt profiling.

### Inventory indexes, per-event player identity, and retained batch timers

Inventory live upserts/deletes now have ID-to-vector-position indexes. Replacing
an upsert is in-place; removals use `swap_remove` and repair the moved row's index.
Repeated deletes coalesce and a later upsert cancels the delete. Payload vectors
still transfer intact to the existing bounded DB queue and leave zero capacity
behind; the small integer-only indexes retain reusable capacity. Notifications
remain per-event and changelog rows remain an ordered, uncoalesced vector. Flush
order remains upserts, changelog, then deletes; independent row order within a
statement is unspecified. Snapshot reconciliation and changelog generation policy
are not changed by this optimization.

XP Update events lazily resolve one stable player slot shared by all boards in
the same RankingSystem. The borrowed identity handle validates registry ownership
and never caches rank or score; per-board membership and publication checks still
run for every rank read. Unchanged events do not register unused slots. Previous
rank reads used only by debug logging are gated by debug enablement; both level
rank reads are skipped when debug is disabled because that branch emits no rank
notification. Notification rank reads remain unconditional. Insert/snapshot paths
continue using their existing player-ID APIs.

BatchTimer keeps one pinned, boxed Sleep for each active batch instead of creating
and dropping a Sleep after each cancelled select branch. Idle batches have no
Sleep; the first message arms one fixed deadline, and later messages never reset
it. The timer is released when the batch ends. This trades one allocation per
active batch for less timer registration/cancellation churn. Tests cover timer
identity across cancellation, deadlines, idle/size/shutdown flushes, index repairs,
notification/changelog preservation, and resolved-player publication/ownership.
CPU/allocation trade-offs require a rebuilt equivalent-workload profile.
