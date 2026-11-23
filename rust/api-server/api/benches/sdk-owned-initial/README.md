# Skip owned-initial dispatch on live SDK updates

**Keep the SDK guard.** On both tested cores the release replay retired **1.73%
fewer user instructions** in every pair, about **147 fewer per transaction**
across five passes. User cycles fell in 22 of 32 pairs. Thread CPU time was
close to flat; the measurements do not establish an API CPU or p99 improvement.
Both builds delivered **245,643 callbacks** for **104,116 transactions** with
ordered checksum `bb0c984258351b7a`. Every transaction reached a listener.

| Core | Metric | Baseline median | Guard median | Median paired change | Pairs lower |
|---|---|---:|---:|---:|---:|
| 11 | Thread CPU ms / 1,000 tx | 0.7492 | 0.7509 | −0.64% | 9/16 |
| 3 | Thread CPU ms / 1,000 tx | 0.6775 | 0.6719 | −0.81% | 10/16 |
| 11 | User cycles, billion / process | 1.732 | 1.724 | −1.65% | 11/16 |
| 3 | User cycles, billion / process | 1.544 | 1.522 | −0.81% | 11/16 |
| 11 | Burst p99, µs | 45.10 | 44.08 | −5.51% | 11/16 |
| 3 | Burst p99, µs | 28.79 | 30.30 | −1.28% | 9/16 |

The paired change compares adjacent runs; it need not equal the ratio of the
two independent medians. On both cores the median user instruction count per
process was 4.424B → 4.347B. Profiling one additional `cpu` invocation per
build with `perf record -F 4999 -e cycles:u` on core 11 found 37 baseline leaf samples (1.35% of sampled
user cycles) in the generated dispatcher and zero there on the candidate.
[Core 11 samples](samples.jsonl) · [core 3 confirmation](core3.jsonl).

## Why this works

In the debug API trace `/tmp/opencode/api-decoder-writers.2bxe34ud`, 15 of
2,759 stacks contained `DbUpdate::invoke_owned_initial_callbacks` (1.04%
of user cycles, weighted by sample period).
The regional bindings sort present tables into callback order, dispatch by table,
then sort them back into apply order. Only *inside* that dispatch does the SDK
check whether a table is initial.

The [SDK change](guard.patch) checks the event kind in `DbContextImpl::apply_update`.
Only `SubscribeApplied` marks the typed update initial. Every update still applies
its diff and runs borrowed row callbacks; only a subscription then needs the
generated owned phase. In particular, an empty table present in a subscription
still takes that phase. No binding regeneration is required.

## Method

* SDK `6beb46da7`, regional bindings `1184a055`, cacheless Rust SDK with the
  API's Rustls and streaming-decompression features. `rustc 1.100.0`, mimalloc
  v3, opt-level 3, release LTO and line tables in both builds. Only the SDK
  guard differs. The replay-only hook hands a typed transaction to
  `process_message(ParsedMessage::TransactionUpdate(...))`.
* [sdk-live](../sdk-live/README.md) reads all 104,116 uncompressed transactions
  from the private `region14.wire` capture (83.5% of the recording's frames;
  20,521 compressed transactions are excluded). Timed work starts with a resident wire
  payload and includes envelope and generated row decoding, apply, borrowed
  callback delivery, owned dispatch if applicable, and destruction. No network,
  DB writes, or API handler bodies. Only ordinary row listeners are registered
  in timed runs: baseline rejects non-initial tables before it looks up an
  owned listener. `check` hashes event kind, table and old/new BSATN values
  outside the timing loop.
* Each CPU sample is a fresh process pinned to CPU 11 (32 MiB L3) or CPU 3
  (96 MiB L3) on a shared Ryzen 9 9950X3D: warm one pass, measure four
  passes of current-thread CPU.
  CPU time and instruction/cycle counts come from
  the same invocation; `perf stat` covers the **whole process** including
  loading and the warm pass. A separate process computes last-callback-entry
  p99 from decode start and from the start of a 32-message burst. Sixteen
  pairs alternate base-first and candidate-first. Report medians of the
  per-process measurements; latency is not a pooled p99.
* [Lifecycle check](lifecycle.rs): the public regional client opens two actual
  loopback WebSocket connections to a [synthetic peer](lifecycle.js). It
  asserts callback sequence, event kinds and row values. Baseline and candidate,
  each tested with and without `client-cache`, observed
  `connected → applied → both borrowed initials → both owned initials → live insert →
  reducer update → reducer completion → unsubscribe ended → delete(s) →
  disconnected`. Initial tables are sent in reverse callback order; one
  table is always empty, the other empty on the first connection and
  populated on the next. This models application reconnect by opening a
  new SDK connection.

## Reproduce

From this directory on Linux with Cargo's dependencies cached, Bun, `perf`,
`taskset`, the two checkouts and the private capture:

```sh
python3 build.py "$SDK_REPO" "$out"        # fresh directory, includes both loopback checks
python3 measure.py "$out" "$WIRE" 11 > samples.jsonl
python3 summary.py < samples.jsonl
python3 measure.py "$out" "$WIRE" 3 > core3.jsonl
python3 summary.py < core3.jsonl
```

From the SDK fork, the candidate's targeted library tests:

```sh
cargo test --locked --offline -p spacetimedb-sdk --lib --no-default-features --features streaming-decompression,tokio-websockets-rustls
cargo test --locked --offline -p spacetimedb-sdk --lib
```

35 cacheless and 37 default-feature tests passed (one ignored pre-existing
microbenchmark in each). These include borrowed/legacy/owned order, empty
and repeated batches, row-byte retention, duplicate keys and cache semantics.
