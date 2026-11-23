# Inside `DbUpdate::try_from` on live transactions

**The largest component is decoding owned typed rows, not finding the table
by name.** On SDK `7303225f0`, the intact generated `try_from` took **243 ns
of CPU per transaction**. Isolated probes took **29 ns for name lookup,
205 ns for typed rows, and 34 ns for ordered placement**.

## First comparison

Of 232,860 table updates, 147,368 could be pushed onto the end, while 85,492
had to be inserted earlier to preserve apply order. None required merging
duplicate table entries on this capture. 67,367 of the 104,116 transactions
touched exactly two tables. 430,164 of 456,193 row lists contained one
fixed-size row.

| Variant | Intact `try_from` ns/tx | Typed rows probe ns/tx | Full CPU ms/1,000 tx | Burst p99 µs |
|---|---:|---:|---:|---:|
| Baseline | 243 | 205 | 0.806 | 42.29 |
| Single-row decode | 232 | 192 | 0.785 | 40.05 |
| Route via typed parser function pointer | 265 | 207 | 0.825 | 43.47 |
| Both | 244 | 190 | 0.800 | 43.24 |
| Special-case empty/one-table placement | 247 | 207 | 0.810 | 41.45 |

These are medians from ten rotated/reversed blocks on core 11 ([raw samples](samples.jsonl)). Every variant
delivered the same 245,643 callbacks with ordered checksum `bb0c984258351b7a`.
Separate fixtures checked repeated table chunks and keys across query sets,
noncanonical table order, unknown names and a truncated typed row.

* **Row:** when a cacheless destination vector has zero capacity and the
  incoming list contains exactly one whole row, decode directly from the borrowed
  row span into a one-element owned vector. This bypasses row-list iteration
  and reserves space for one row; it still allocates that vector. No change to wire format, unknown-table
  handling, cache-enabled mode or retained-row-byte policy. Output rows are
  still fully owned. The intact conversion improved in 10/10 pairs; CPU
  through callbacks improved in 9/10. Burst p99 improved in only 6/10.
  Repeated appends can regrow a one-row vector; the duplicate-table fixture
  checks their semantics, not throughput for a duplicate-heavy workload.
* **Route:** keep the full string match but select a typed parser and perform
  one common append after its indirect call. Although the generated
  `try_from` symbol shrank from **82,415 to 20,109 bytes** in this diagnostic
  build (excluding the new parser wrappers), its CPU time increased.
* **Placement:** an explicit first/second-table path cut the isolated
  placement median from 33.8 to 31.3 ns; the full path did not consistently
  improve. Keep the existing generator.

## Measurement boundary

**Release-LTO confirmation:** with the diagnostic methods removed, median
CPU through callbacks was **0.768 → 0.732 ms per 1,000 transactions** (4.7%
lower, 6/8 paired blocks). Burst p99 was **39.52 → 37.82 µs** (5/8).
The CPU improvement is modest, and the latency ranges overlap; these are
replay results, not an API-process or production-p99 saving. The same callback
checksum and error/merge fixtures passed under LTO. [LTO samples](lto.jsonl)
and [tested SDK patch](single-row.patch).

The live local API profile attributed **12.6% of sampled user cycles** to a
stack containing the generated transaction conversion (`4.3%` with that
function as the leaf). That includes both table dispatch and typed row work.
This experiment uses the same region-14 uncompressed transactions as the
[callback replay](../sdk-live/README.md), with SDK `7303225f0` (shared contexts).

`phases` prepares owned `TransactionUpdate` envelopes **outside** the timers
in chunks of 256 messages. It times the actual, unsplit `DbUpdate::try_from`
from input consumption to completed typed updates. Output destruction is
outside that timer. Three other probes explain the work:

* **Name lookup:** resolve each borrowed name with the generated 329-name
  match to produce a numeric ID. No row decoding.
* **Typed rows:** use precomputed IDs and the real generated per-table parser
  to produce the typed enum variants. Includes consuming/dropping raw tables.
* **Placement:** append pre-parsed variants into one ordered `DbUpdate` per
  transaction, including its capacity reservation and any duplicate merge.

These are diagnostic loops with intermediates materialized at different
boundaries; **their times aren't additive**. The acceptance measures are
separate uninstrumented CPU passes from complete wire payload through the
last callback and destruction, and a burst-p99 pass with 32 messages ready
together. No timers in the generated production code. The output's `*_ns`
phase times are client CPU **per transaction**, not per table or per row.

## Reproduce

From this directory on Linux, using the private `region14.wire` fixture
described in the [decode replay](../sdk-decode/README.md). Use an empty
output path and a dedicated available core. The scripts build patched SDK
and binding copies; the diagnostic methods exist only in those copies.

```sh
python3 build.py "$SDK_REPO" "$out"
python3 measure.py "$out" "$WIRE" 11 > samples.jsonl
python3 summary.py < samples.jsonl
```

To confirm the row candidate without any diagnostic methods in the bindings,
use the API's `lto=true`, `opt-level=3`, line-tables-only release settings:

```sh
python3 build.py "$SDK_REPO" "$lto_out" --confirm row
python3 measure.py "$lto_out" "$WIRE" 11 --confirm row > lto.jsonl
python3 summary.py < lto.jsonl
```
