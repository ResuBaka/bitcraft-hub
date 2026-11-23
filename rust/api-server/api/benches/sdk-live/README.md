# Uncompressed live transactions: SDK CPU through callback delivery

**Keep the [shared connection context](shared-context.patch). Reject the
[inline row-event experiment](inline-events.patch) for this workload.**
The context change alone used **32.7% less CPU** and cut ready-burst p99
**28.2%**, winning both metrics in all eight paired blocks.

| Variant | CPU ms/1,000 transactions | Burst-to-last-callback p99 | Decode-start-to-last-callback p99 | Allocation calls per capture |
|---|---:|---:|---:|---:|
| SDK `9e997ce63` | 1.117 | 42.35 µs | 1.90 µs | 2,792,155 |
| Inline row events | 1.172 | 43.89 µs | 2.08 µs | 1,929,701 |
| **Shared context** | **0.752** | **30.43 µs** | **1.69 µs** | 2,792,155 |
| Both changes | 0.772 | 31.67 µs | 1.78 µs | 1,929,701 |

The winning change saved **0.365 ms of client CPU per 1,000 messages** without
lowering allocations: it removed redundant reference-count operations. Baseline
CPU ranged from 1.103–1.123 ms/1,000 across processes; shared context ranged
from 0.743–0.769. Inline events made **862,454 fewer allocations** but cost
**5.0% more CPU** than baseline; the combination also lost to shared context
on median CPU. Keeping the original event vectors avoids that tradeoff.

All four builds delivered **245,643 callbacks** for 104,116 messages with
the same ordered content checksum `bb0c984258351b7a`. Every measured
transaction reached a listener. [Raw samples and counts](samples.jsonl).

## What the replay measures

* The region-14 recording contains **104,116 uncompressed transaction
  messages**. We keep every one in order. They produce 232,860 table changes
  after grouping repeated tables within a transaction: **209,863 (90.1%)**
  contain one insert and one delete. The busiest tables are player actions
  (99,364), mobile entities (42,109) and progressive actions (33,930).
* The starting point is a complete compression-tagged WebSocket payload in
  memory. A private SDK fixture invokes the normal envelope decoder, generated
  regional `DbUpdate` parser, `apply_update`, primary-key matching and row
  callback dispatch. Insert/delete/update listeners cover all 18 captured
  tables (insert-only for `craft_event`). The timed listeners consume row
  references; they do not run API business logic or write to PostgreSQL.
* **CPU:** warm up once, then measure current-thread CPU for four full passes
  and normalize to 1,000 input messages. **Latency:** in a separate pass,
  make 32 messages ready at once, process them serially, and take a monotonic
  timestamp inside each callback. Report the p99 time to the **last callback
  entry for each transaction** from (a) burst release and (b) start of its
  own decode. Neither timestamp is the original server send time. A burst
  of 32 is a controlled stress case, not a claim about the production queue.
* Four separately compiled variants use the same SDK base (`9e997ce63`),
  generated bindings, cacheless features, allocator (API's mimalloc v3),
  capture and release profile. Each runs as a fresh process pinned to core 11
  on a Ryzen 9 9950X3D. Eight paired blocks rotate and reverse execution
  order. A separate instrumented executable counts client
  thread allocations including reallocations; its CPU time is not used.
* `check` independently hashes the ordered callback stream (event kind,
  table, and BSATN for every old/new row) outside timing. The runner checks
  counts and checksum across builds and rejects missing latency observations.

### Production interpretation

The initial profile of the child replay identified connection context clone/drop
and allocator work. A subsequent **live local API** profile on 2026-11-23
successfully attached to the running debug binary (SDK checkout `7303225`,
verified in the executable's DWARF). This is a post-change sample, not an A/B
against the previous SDK or a measurement of the four-core release deployment.

* `perf record -e cycles:u -F 199 --call-graph dwarf,4096 -p PID` across the
  process for 90 s: **2,856 samples, none lost**. `/proc/PID/stat` advanced
  by **7.02 user + 1.71 system CPU seconds** (0.097 cores on average).
* A separate matched 60 s counter interval saw **112,882 upstream WebSocket
  messages**, **0.74 MiB/s** of payload and **5.54 CPU seconds**, or **49.1 ms
  of whole-process CPU per 1,000 incoming messages**. All five scrapes showed
  ten connected databases. Worker sent/received counters each advanced by
  136,022. WebSocket messages include more than just uncompressed transactions,
  and process CPU includes unrelated tasks: don't compare 49.1 directly with
  the replay's 0.752.

| Live path | User cycles with a matching stack frame |
|---|---:|
| SDK `apply_update`, including application callbacks | 15.4% |
| Generated typed `DbUpdate::try_from`, including row parsing | 12.6% |
| Borrowed row callback dispatch, including callback bodies | 9.6% |
| TLS stack, including underlying reads | 7.4% |
| gzip inflate (`flate2` / `zlib_rs`) | 5.4% |

Shares are weighted by `perf` sample period. **Rows overlap** (e.g. callback
dispatch is inside `apply_update`); these are on-CPU user cycles, not wall-time
or blocked I/O. `DbUpdate::try_from` itself was the largest individual leaf
(4.3%). The separate `DbContextImpl` clone/drop symbols that stood out in the
baseline replay no longer appeared as sampled leaves; inlined work is not
separately attributed.

The [follow-up `DbUpdate::try_from` replay](../sdk-try-from/README.md)
separates table-name dispatch and ordered append from typed row decoding,
then measures candidates through callbacks on the same transaction mix.
For deployment impact, compare release builds at a matched message mix and
rate, recording CPU and queue lag.

## What changed

1. **Inline row events:** keep one callback reference entry inline on each
   side of a diff, spilling larger lists to heap storage. Store matched old/new
   references in pairs rather than parallel vectors. The SDK still matches
   duplicate keys FIFO and preserves callback order.
2. **Shared context:** the generated event context carries connection handles
   for the database, reducers, procedures and itself. Cloning each handle
   previously cloned each independently shared lock/channel/runtime too.
   Share the bundle with one `Arc` per connection; handle clones increment
   one reference count. This adds one allocation at connection creation,
   not per message. The underlying locks, channels and dispatch rules stay
   the same.

## Reproduce

On Linux, with the SDK fork and the private `region14.wire` from the
[SDK decode replay](../sdk-decode/README.md). No capture contents are
published. Use a fresh `out` directory and one available physical core.

```sh
python3 prepare.py "$SDK_REPO" "$out"
python3 build.py "$out"
python3 measure.py "$out" "$WIRE" 11 > samples.jsonl
python3 summary.py < samples.jsonl
```

`prepare.py` copies the bindings and app lock, archives the specified base,
and adds the offline hook to the private SDK only. `build.py` toggles the
two SDK diffs in that copy. It expects Cargo's dependencies to be cached.
Each output row is either a correctness/allocation summary or one paired
timing sample. Compare medians **within a metric**; burst and service p99s
are medians of per-process percentiles, not pooled request percentiles.
