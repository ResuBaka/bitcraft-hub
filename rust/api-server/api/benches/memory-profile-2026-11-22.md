# API memory profile — 2026-11-22

**The quickest RSS win to validate is `MIMALLOC_PURGE_DELAY=0`: the normal
release used 509 MiB less resident memory in a matched post-sync window on
one run per setting.** For code changes, start with inventory snapshot writes
for peak memory and the spatial claim index for persistent memory. In the
six-minute ingestion capture the API
peaked at **832.4 MiB of live heap** around 128 s. At 350 s it held **239.1
MiB**, of which **115.9 MiB (48%)** came from the claim-location index. These
are different problems: optimizing a snapshot decoder won't remove that index,
and compressing the index alone won't fix the startup peak.

## Workload and interpretation

This profile is for the Rust API at `9d89101e`, including the batched ingestion
workers. The workload is an API restart against an existing database, followed
by live upstream ingestion. There is no frontend, downstream WebSocket fan-out
or representative HTTP request mix; HTTP traffic consists of metrics scrapes.

The test uses private copies of the existing claim-sorted benchmark database
(9,827 players and 196,520 experience rows at the start). Ingestion writes go to
the copies, not to the normal API database. The live upstream changes between
runs, so this is not a deterministic replay.

For historical context, `ab9fe51a` reported 492 → 257 MiB settled heap, and
`1dfa1833` reported 175.9 → 107.2 MiB **for the claim index**. Today's 239.1 MiB
whole-process sample and 115.9 MiB index measurement are in that scale, but
they are not a commit-to-commit regression test: the upstream world has changed
and one region reconnected in this capture.

Heaptrack 1.5.0, Linux x86-64, Rust 1.100, locked release build with line
tables, default Rustls transport and Tokio worker queues. One global plus nine
regional databases (7–9, 12–14, 17–19); state subscriptions enabled, inventory
and vault initial batches of 1,000, DB worker flush window 2 s, pool maximum
200. The six-minute timer includes startup. Another profiling process was
active on this host against a different database; throughput and reconnect
timing are not controlled benchmarks. For the profiled run, RSS below is from
the trace or the target's own shutdown markers.

The following measurements answer different questions:

* **Live heap** is the sum of outstanding allocation requests intercepted by
  Heaptrack. Measure it before shutdown to identify state the service keeps.
* **Peak heap** is the largest *simultaneous* live total. Adding each stack's
  lifetime maximum does not give this number.
* **Gross allocated bytes and call counts** include memory allocated and freed
  repeatedly. Reducing these can improve throughput without reducing live heap.
* **RSS** includes allocator-retained pages, executable mappings, stacks and
  other resident memory. Only compare release-process RSS at equivalent sync
  progress. PostgreSQL and the external profiler are separate processes.

## Six-minute ingestion capture

| Point in the run | Live heap (MiB) | Profiled process RSS (MiB) |
| --- | ---: | ---: |
| 30 s | 96.7 | 173 |
| 60 s | 131.9 | 249 |
| 120 s | 710.1 | 3,539 |
| Exact heap high-water, ~128 s | **832.4** | ~3,675 |
| 180 s | 400.0 | 3,495 |
| 240 s | 235.8 | 3,465 |
| 300 s | 223.0 | 3,470 |
| 350 s | 239.1 | 3,486 |

Late heap was **223–252 MiB** across timestamp samples from 300–355 s
(median **232 MiB**). All ten connections were up in the late scrapes.
One region reconnected, and one skill-description reconciliation logged a DB
pool timeout; this is a real ingestion run including recovery, not a pristine
single-pass sync. By 143 s both inventory and vault had read ten completion
markers each, with no outstanding private chunks. The location writer still
had 24,000 queued rows then; by 183 s all 3.76 million enqueued location
upserts had been received. Completed subscriptions are not completed writes.
The API drained its workers and reported graceful shutdown.

**74,988,273 allocation calls, 26.85 GiB requested cumulatively**, 4,747,963
temporary allocations by `heaptrack_print`. The terminal outstanding total
was 7.94 MiB; six minutes and an exit total don't establish a leak. At shutdown
the profiled process reported **3.60 GiB high-water RSS**, **3.41 GiB current
RSS**, and still **3.35 GiB after state drop**. A late `smaps` sample attributed
~3,130 MiB to anonymous mappings and ~333 MiB to `[heap]`. This shows why
“RSS minus live heap” should not be called an application cache or a measured
allocator fragmentation saving.

### Normal release: the resident-memory baseline

On a fresh clone, the uninstrumented **mimalloc** release applied all nine
regional subscriptions in ~36 s without a retry. Inventory/vault had read all
18 completion markers by the 40 s scrape. The process reported **1,755 MiB
high-water RSS by shutdown**, **1,198 MiB RSS at shutdown request**, and
**1,161 MiB after draining and dropping state**. Median RSS over 300–355 s was
**1,250 MiB**. The difference from the profiled RSS is substantial. Allocator,
instrumentation, timing and the extra regional snapshot all differ; do not
call that difference “memory saved by mimalloc.”

**A configuration experiment on the same release binary:** I repeated startup
against a third fresh clone with `MIMALLOC_PURGE_DELAY=0` (immediate purge;
verbose allocator logging confirmed the option). This run stopped at 240 s.
The table compares only pre-shutdown windows covered by both runs.

| Same-binary release measurement | Default | Immediate purge |
| --- | ---: | ---: |
| Median RSS over 180–235 s (55 samples each) | **1,287 MiB** | **778 MiB** |
| Min–max RSS in that window | 1,247–1,303 MiB | 772–814 MiB |
| All nine regional subscriptions applied | 36 s | 32 s |
| Inventory/vault completion markers by 40 s | 18 | 18 |
| Location rows enqueued/received balanced by 80 s | 3.40M | 3.40M |
| High-water RSS reported at shutdown (360 s / 240 s) | 1,755 MiB | 1,389 MiB |

That's **509 MiB (40%) lower median RSS** with no application change; the
high-water marks were already reached in the first minute in both runs. Both
drained gracefully. This changes when free pages are returned to the OS, not
the size of live data structures. Live traffic, scheduling and fresh process
histories differ; CPU, page faults and client-facing p99 were not benchmarked.
**Try it as a canary**, alternating runs on matched input and comparing CPU,
minor faults, readiness, sustained throughput and p99 before making it the default.

If the operational goal is lowering a container memory limit, use **native
high-water RSS and late native RSS** as acceptance criteria. After reducing
snapshot work, check whether RSS follows. If not, collect mimalloc committed,
purged and live statistics plus `smaps_rollup`, and compare worker counts and
purge policy one at a time. A larger fallback is a short-lived snapshot builder
that streams a compact result to the long-lived service and exits, allowing
the OS to reclaim its temporary working set. This requires a consistent
snapshot-to-delta handover, replay/catch-up on failure, and measuring the
**combined** cgroup peak while both processes are alive. Splitting processes
without removing the overlap can increase the required memory limit.

### What was live at the *same instant*?

These are exclusive allocation-stack groups, measured at the event that set
the global peak and at the 350 s checkpoint. Groups describe **where memory was
allocated**, even if ownership subsequently moved. Figures are in MiB.

| Origin | At peak | At 350 s |
| --- | ---: | ---: |
| SQLx parameter-type vectors (`resolve_statement_metadata`) | 139.5 | 16.7 |
| SQLx PostgreSQL Bind encoding/write buffers | 134.2 | 4.6 |
| SQLx prepared-statement cache insertion | 33.0 | 3.7 |
| SeaORM statement construction (`statement.rs:102`) | 42.4 | 0.1 |
| SDK `TableUpdate::append_rows` merging `location_state` | 160.6 | 0.0 |
| Spatial claim index and its auxiliary maps | 115.5 | 115.9 |
| Compact leaderboard pending-map reserve + other compact leaderboard | 24.7 | 24.7 |
| Equipment/preset and character-stat row payloads allocated by SDK decode | 17.5 | 17.4 |
| Tracing queue | 3.9 | 3.9 |
| Remaining stacks, including other DB/ORM and application allocations | 161.1 | 52.1 |
| **Total** | **832.4** | **239.1** |

## Changes worth making

### 1. Bound inventory snapshot writes by bytes, then test fixed-shape upserts

Follow-up implementation and measurements: [inventory snapshot writes](inventory-snapshot-writes.md).

The four SQL/ORM groups above account for **349.1 MiB, or 42% of peak**.
Looking down those *same stacks* to the application call sites attributes
**183.9 MiB** to `inventory/bitcraft.rs:266`, **86.5 MiB** to generic
`websocket/initial_sync.rs:255`, and **38.3 MiB** to
`vault_state/snapshot.rs:109`. These are subsets of 349.1 MiB, not additional
savings. Location's existing fixed-shape writer accounted for only 4.6 MiB
of those groups. Pool size reached 33 in the 143 s scrape and fell to 3 by
283 s; big queries leave parameter metadata, SQL keys and buffers on multiple
connections even after a batch is finished.

There is also a **separate 41.8 MiB peak stack** in the “remaining” row:
SeaORM's PostgreSQL `sqlx_query(&Statement)` clones `stmt.values` in
`sqlx_postgres.rs:350`. This matters for pockets: large JSON-containing bind
values can exist both in the statement and in the driver's owned arguments.
A fixed SQL string alone won't prevent that copy. For the hot writer,
compare direct typed SQLx binds against fixed SQL through SeaORM, keeping the
same connection pool and query/latency metrics.

* **First experiment: byte-limited writes.** Keep the row cap and the existing
  changed-row comparison. Flush before adding a row that would exceed an
  estimated encoded-byte budget, including variable-sized JSONB pockets. Do
  not build a whole SQL statement or a second JSON buffer just to size it.
  Send a single oversized row alone and count it. This bounds normal statement
  payloads, not total queue or comparison memory. Smaller batches can add
  round trips and autocommits, and varying row counts still produce different
  SQL strings. Select the budget by measuring peak heap **and** time to drain.
* **Second experiment: fixed-shape array upserts.** Keep the same byte and row
  caps; bind one typed array per column to `INSERT ... SELECT FROM unnest(...)`,
  including a `jsonb[]` for pockets, following `location_upsert_statement`.
  Keep the `entity_id` conflict key and existing update columns. Use
  non-returning execution where the result is unused. This isolates the
  benefit of fixed parameter count from the benefit of smaller batches.
* **Next:** apply to vault and the largest concrete users of generic
  reconciliation. At 350 s the remaining SQLx groups are mostly *live* writers:
  inventory (~6.4 MiB), crafting (~5.5), and two XP paths (~6.5 combined).
  Don't rewrite every repository query or assume startup-only fixes cover them.
* **Larger change:** for large snapshots, stream rows via COPY to a temporary
  staging table, then merge changed rows into the target. Keep COPY and merge
  on the same checked-out connection. Temporary staging avoids WAL for the
  staged row data; a regular logged staging table adds a second logged write
  path. The durable target still generates normal heap, index and potentially
  TOAST WAL. If staging the whole snapshot, filter unchanged rows in the merge
  (`IS DISTINCT FROM` on the updated columns); otherwise even identical
  conflicts can create new row versions and vacuum work. A large merge can
  hold target row locks longer and concentrate WAL/replica lag into a burst.
  Release staging on an incomplete snapshot; authorize stale-row removal only
  after successful completion. Preserve live-update ordering, JSON equality,
  conflict keys and the current no-deletion contract for partial location
  subscriptions. Compare on fresh clones with the same input: API peak heap,
  time to drain, rows actually inserted/updated, WAL bytes, lock waits and
  peak staging space. `pg_stat_wal.wal_bytes` deltas are cluster-wide, so use
  an isolated PostgreSQL instance for that comparison.
* As a cheap diagnostic, A/B a smaller SQLx statement-cache capacity independently
  of pool size. Track DB acquisition waits, errors, cache misses/reprepares and
  time to drain. The observed timeout argues against blindly capping connections.

### 2. Deliver decoded location rows incrementally

**160.6 MiB at peak, zero at 350 s** came from growing the combined location
insert vector in the pinned SDK (`spacetime_module.rs:285–295`, generated
`game_module/src/module_bindings/mod.rs:11917`). Multiple subscribed joins
return the same table. The SDK adopts the first vector but appends subsequent
ones into a contiguous `Vec<WithBsatn<LocationState>>`. Application chunking
after that point is too late. The cacheless build still carries an empty
`Bytes` field in each `WithBsatn` row.

* First, specialize the cacheless row representation and preserve per-query
  vectors as segments instead of growing one combined allocation. Measure
  **live bytes and simultaneous segments**, not just fewer reallocations.
* For the bigger gain, stream bounded row batches from decode through both
  consumers (claim index and location writer). Use a byte budget across regions
  and producer/consumer hand-off that can pause without blocking a synchronous
  callback on its own runtime. Regional startup is **already serialized through
  subscription application**; if introducing a gate, gate on outstanding bytes
  or writer progress, not another connection semaphore.
* Preserve order, overlapping join semantics, empty snapshots, reconnect resets
  and one explicit end-of-snapshot marker. Inject a mid-snapshot failure and
  verify no stale deletes and no false “unclaimed” result. Test `location_db_*`
  queue depth and readiness time. The 160.6 MiB is an opportunity size, not the
  amount freed by segmenting: consumers still need some decoded data.

There is a related **warm-restart** opportunity below the SDK. The location
writer unconditionally upserts initial rows in 3,000-row batches. I compared
all six stored columns by primary key between the starting fixture and the
post-run DB: **3,112,659 identical**, 9,012 changed, 298,347 only after and
113 only before. This is net state, not proof that every identical row was sent
or never changed during the run. Try an `ON CONFLICT ... DO UPDATE ... WHERE`
row-distinct predicate to avoid rewriting identical DB tuples. It still sends
every row; to reduce API allocation volume too, send typed columns directly
instead of constructing `ActiveModel`s, or compare chunk versions upstream if
the source can guarantee them. Location model batch growth alone requested
860.5 MiB cumulatively. Verify resulting DB contents and the update/delete
ordering, plus WAL, queue lag and API RSS.

### 3. Compact the spatial claim index for the biggest lasting reduction

`claims/claim_location.rs` already uses a sorted base plus a bounded edit
overlay. At 350 s, **75.3 MiB** was the ID-sorted base, **25.1 MiB** a second
copy of the coordinate keys, and **12.55 MiB** the coordinate-order `u32`
slots. That's ~3.29 million tiles at **36 bytes/tile**; auxiliary allocations
bring the total to 115.9 MiB. The `/metrics` legacy `claim_tile_state` and
`location_state` cache sizes were both zero; neither measures this index.

* **Prototype A:** drop the duplicate coordinate array and binary-search the
  sorted slot array by reading coordinates from the base row. Payload budget:
  **25.1 MiB**; cost: less-local reads on a hot lookup. Compare p95/p99 claim
  lookups during normal reads and during a region rebuild.
* **Prototype B:** store the base as separate arrays: 64-bit entity ID, 32-bit
  packed local coordinates, 32-bit claim dictionary slot, 32-bit permutation.
  That is **20 vs 36 bytes/tile**, a modeled **50.2 MiB** smaller base before
  dictionary/fallback costs, **including** prototype A's saving. As a
  feasibility check, the local DB's dimension-1 locations in the nine regions
  all fit two 16-bit region-relative axes in this run. This is a *sample*,
  not a world-coordinate invariant: range-check on build, retain a wide fallback
  for outliers/moves, and preserve negative coordinates and conflict semantics.
* **Prefer a coordinate-sorted compact base if lookups dominate edits.** Lay
  out coordinates, entity IDs and claim slots in coordinate order; keep a
  single `u32` permutation in ID order for edit lookup. This keeps the same
  modeled 20 bytes/tile but restores contiguous coordinate searches.
* **Larger option:** a per-region packed spatial snapshot or database-backed
  lookup with a small in-memory overlay could avoid most of the 113 MiB base
  on the API heap. Benchmark cold and hot lookups, rebuild peak, freshness and
  total RSS/page cache. A memory-mapped index still consumes resident pages
  when touched; only consider external storage if a ~50 MiB layout win is
  insufficient. Add bytes/rows gauges for the real index first.

**I tested the layout trade-off.** An isolated Rust `-O` prototype used all
491,136 dimension-1 location IDs/coordinates from region 12, stand-in claim
IDs, identical lookup sequences and verified checksums. Half the coordinate
queries picked a real key; half perturbed a key. Median of seven warmed passes
of 500,000 queries, rotating test order, on the shared host:

| Base layout | Payload B/tile | ns/coordinate lookup |
| --- | ---: | ---: |
| Current shape: ID-sorted rows, separate sorted keys and slots | 36 | 132 |
| Same rows, drop duplicate keys, indirect coordinate search | 28 | 209 |
| Packed columns in ID order, indirect coordinate search | 20 | 183 |
| **Packed columns in coordinate order**, indirect ID search | **20** | **117** |

The last layout's ID lookups took **195 vs 145 ns** for the current shape. The
packed cases additionally used a 32 KiB test dictionary. This measures the
static lookup kernel, not the live API: it excludes overlays, dictionary
construction, region locking, outliers and rebuilds. It argues for measuring
the real lookup/edit ratio before implementing the seemingly simpler 28-byte
variant. If it holds, the 20-byte layout would remove about **one fifth of the
350 s live heap** without moving work into a database.

### 4. Treat allocation churn as a separate throughput project

Streaming frame decode stacks requested **8.83 GiB cumulatively**, but held
only ~0.1 MiB at the global peak. `GzDecoder::new` and the 16 KiB reader were
large contributors. Prototype reusable decoder scratch **per connection** with
a retention cap, reset each independent gzip member and preserve error handling.
Replay the same frame capture and check allocations/message and CPU. This is
not an 8.83 GiB RSS opportunity.

SeaORM statement building and SQLx argument adaptation also dominate calls:
two individual stack groups account for **13.75M and 13.72M calls**, respectively.
Avoid rebuilding/cloning large value trees in inventory writes before tuning
small event allocations. Equipment/preset payloads (~13.8 MiB late), character
stats (~3.6 MiB) and individual small caches are second-tier retention targets.

## Database-only startup

`profile-init --settle-seconds 5` completed in 9.82 s under Heaptrack. Loading
the caches took 4.77 s from the first application memory marker. It processed
196,500 eligible XP rows, registered 9,826 players on 25 boards and had zero
pending ranking changes at the final marker. Peak requested heap was **40.12
MiB**, settled heap at 8 s was **40.02 MiB**, and final-state RSS was **80.90
MiB** before the caches were dropped. There were 836,560 allocation calls
(215,676 temporary) requesting 126.20 MiB in total.

| Allocation origin, disjoint groups at 8 s | Live MiB |
| --- | ---: |
| Leaderboard pending maps, `leaderboard/compact.rs` `reserve_players` | 12.89 |
| Other stacks passing through leaderboard code | 6.80 |
| Nonblocking tracing queue, `setup_tracing` | 3.91 |
| Everything else, including recipes, descriptions and player caches | 16.42 |

A specific low-risk experiment is to shrink the **post-load** ranking pending
maps once the initial burst has been published, then size them for live updates.
The application-reported board vector capacity of 6.05 MiB omits the hash
tables. `reserve_players` reserves pending entries on every board; **12.89 MiB
is the measured allocation budget, not a promised saving**. Simply deleting
`reserve` can still grow the maps back during initial load. Compare after a
publication cycle, watch `leaderboard_pending_change_capacity`, and test
publication lag and reallocation under an XP burst. Reducing the logging queue
is a smaller opportunity; the current non-lossy logger can block producers if
the sink falls behind, so test a slow sink before reducing its capacity.

## How to decide which change ships

1. **If restarts hit a memory limit:** validate immediate purge first. For
   reducing actual allocation demand, A/B the inventory writer, then the
   SDK location hand-off. Record peak live heap, native `VmHWM`, time of the
   last region becoming ready, and time to drain location and DB write queues.
   A smaller peak caused by processing fewer regions is not a win.
2. **If the always-on service is too large:** A/B the coordinate-major index
   and the post-load leaderboard trim separately. Check late heap at equal cache
   row counts, native RSS, lookup/update latency, overlay rebuild peaks and
   semantics against the current index. Add per-region rows/capacity/bytes
   metrics so population growth can be normalized.
3. **Before extrapolating:** repeat the winner on fresh clones or the same
   recorded upstream input, then soak through several pool lifetimes and a
   forced reconnect with normal HTTP/WebSocket clients. Compare query latency,
   downstream queue bytes, DB content and graceful drain. This capture does
   not measure slow-client fan-out or long-term growth.

## Reproduction and evidence

From `rust/api-server` on a machine with the local benchmark DB and upstream
configuration. Restore the same migrated fixture into a **fresh database per
serve run**. `serve` writes to it. The `.env` supplies the upstream settings;
override the DB and listener explicitly:

```sh
: "${PROFILE_DATABASE_URL:?set to a private benchmark clone}"
export BITCRAFT_HUB_API__DATABASE__URL="$PROFILE_DATABASE_URL"
export BITCRAFT_HUB_API__HOST=127.0.0.1 BITCRAFT_HUB_API__PORT=18176
umask 077
out=$(mktemp -d)
CARGO_TARGET_DIR=target/heaptrack cargo build --release --locked \
  -p bitcraft-hub-api --features system-allocator
bin=target/heaptrack/release/bitcraft-hub-api
heaptrack --record-only -o "$out/init" "$bin" profile-init --settle-seconds 5
heaptrack --record-only -o "$out/live" "$bin" serve --shutdown-after-seconds 360
heaptrack_print -f "$out/live.zst" -m0 -n14 > "$out/live.print"
# For the production-allocator control: switch the DB URL to a fresh clone.
cargo build --release --locked -p bitcraft-hub-api
target/release/bitcraft-hub-api serve --shutdown-after-seconds 360
# On another fresh clone, same binary; compare the shared 180–235 s window.
MIMALLOC_PURGE_DELAY=0 MIMALLOC_VERBOSE=1 \
  target/release/bitcraft-hub-api serve --shutdown-after-seconds 240
```

On the machine used for this report the artifacts are in the ignored directory
`rust/api-server/target/memory-profile-20261122-1gdaTp/`: `init.zst`,
`live.zst`, `*.print`, `live.groups`, `live.ranks`, `live.timeline` (ms, live
bytes), `live.rss` (ms, resident 4 KiB pages), release RSS CSVs, metrics
scrapes, shutdown logs, `layout.txt` and checksums. Open `live.zst` in
`heaptrack_gui` to inspect a time range. Raw traces/logs are local, not part
of the repository report.

`analysis/points.cpp` replays the interpreted trace's allocation/free events;
the peak breakdown uses a second pass at the exact event that set the
high-water mark. Its total matches `heaptrack_print` (872,826,306 bytes,
displayed as 872.83M). `analysis/groups.py` assigns each trace to at most one
group.
