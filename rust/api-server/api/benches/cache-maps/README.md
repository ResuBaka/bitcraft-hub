# DashMap, ClashMap, Papaya, and SCC under Heaptrack

This standalone binary uses the system allocator and synthetic owned rows
shaped like item/cargo descriptions. No server, database, or production
dependency changes are needed. All backends use their default settings,
grow from empty, and receive the same rows and deterministic key sequences.

| Case | Workload |
| --- | --- |
| `load` | Populate an empty map and leave it intact for the loaded-memory checkpoint. |
| `mixed` | Four readers do 100,000 lookups each while one writer is scheduled alongside them and applies 5,000 updates. Each lookup uses its own guard/pin. |
| `scan` | Run 64 full scans on a populated map without concurrent writes. |
| `churn` | Run 100,000 updates with a short guard/pin per update. Every fourth update removes and reinserts its key; the others replace in place at the map level. |
| `pinned` | Repeat `churn` with a Papaya read pin or SCC HashIndex guard held through the first checkpoint, then release it and apply the same follow-up writes as in `churn`. The lock-based controls do **not** keep an entry locked while writing. This case tests reclamation, not equivalent long-lived read locks. |

From the repository root:

```sh
cd rust/api-server/api/benches/cache-maps
cargo build --release --locked
bin=./target/release/cache-map-profile
out=$(mktemp -d /tmp/opencode/cache-map-profile.XXXXXX)
for case in load mixed scan churn pinned; do
  for backend in dashmap clashmap papaya scc-map scc-index; do
    name="$case-$backend"
    heaptrack --record-only -o "$out/$name" "$bin" "$backend" "$case" \
      >"$out/$name.log" 2>&1 || break 2
  done
done
ls "$out"
```

Use `heaptrack_gui <capture>` for the timeline and allocation stacks, or
`heaptrack_print -f <capture>` for a text report. The `.log` gives the capture
path and labels the phases. Each phase marker is followed by a 250 ms pause
to make it easier to spot in the timeline. Compare:

- **`load`:** peak heap during growth, then live heap at `loaded`.
- **`mixed` and `scan`:** peak heap and allocation count. Repeat **without**
  Heaptrack for timing (for example, `"$bin" papaya mixed 16384 100000 4`).
  Set `CACHE_MAP_PROFILE_NO_PAUSE=1` for timing runs to skip the timeline pauses.
  `slowest_reader_ms` is elapsed time for a whole reader's batch, not
  per-request p99. The mixed checksum can vary with thread scheduling.
- **`churn` vs. `pinned`:** live heap at `churn_complete`, after pin release
  and follow-up writes, and after the map drops. RSS can remain high even
  when live heap falls, because the allocator can retain freed pages.

The optional arguments are `[rows] [ops] [readers]`. Defaults are 16,384 rows,
100,000 operations (64 for `scan`), and four readers. `ops` means lookups per
reader for `mixed` (the writer does `max(1, ops / 20)` updates), scan passes
for `scan`, and updates for `churn`/`pinned`; it is unused for `load`.

This compares whole-row replacement caches, not the claim-member indexes that
mutate inner collections in place. Treat the results as a way to shortlist a
real cache migration, not as a prediction of whole-server RSS or latency.

## Earlier three-way comparison

One run per backend with the defaults above. Peak heap is the single
whole-process maximum reported by Heaptrack, not settled RSS. Each cell
shows peak heap / allocation calls.

| Case | DashMap | ClashMap | Papaya |
| --- | ---: | ---: | ---: |
| `load` | 9.08M / 99,235 | 9.01M / 99,232 | 7.31M / 114,760 |
| `mixed` | 9.19M / 129,308 | 9.19M / 129,307 | 7.35M / 150,138 |
| `scan` | 9.12M / 107,438 | 9.10M / 107,437 | 7.31M / 114,769 |
| `churn` | 9.15M / 705,256 | 9.23M / 705,260 | 7.49M / 828,881 |
| `pinned` | 9.13M / 705,256 | 9.21M / 705,259 | 51.10M / 828,491 |

ClashMap and DashMap had nearly identical heap results. Papaya used about
20% less peak heap in the short-pin cases, but made more allocation calls.
At `mixed_complete`, RSS with Heaptrack attached was roughly the same
across all three (15.5–15.7 MiB). The long pin raised Papaya's peak sharply
despite roughly the same allocation count as ordinary churn, consistent
with delayed reclamation. If testing the actual description caches, keep
pins scoped to synchronous reads and measure RSS separately under the
normal server allocator.

Without Heaptrack, three local runs each (median, 16,384 rows):

| Workload | DashMap | ClashMap | Papaya |
| --- | ---: | ---: | ---: |
| `mixed`, 1M lookups per reader, four readers (slowest reader batch) | 41.91 ms | 45.21 ms | 51.37 ms |
| `scan`, 512 full scans | 165.70 ms | 107.79 ms | 34.42 ms |

ClashMap consistently scanned faster than DashMap in these runs, but the
mixed timings overlapped. A real cache trial should check the relative
frequency of scans vs. lookups and whether related Papaya lookups can
reuse one short-lived pin.

## Testing `inline-more`

The standalone harness exposes `dashmap-inline-more` and `clashmap-inline`
independently. Both forward to the respective crate's `inline` feature,
which enables `hashbrown/inline-more` (additional inlining hints). They
affect separate versions of `hashbrown` (DashMap uses 0.14; ClashMap uses
0.15), so enabling both in the same comparison build doesn't unify their
features. Neither flag is on by default or changes the server build.

From this directory, build separate binaries and run timings without
Heaptrack. The first line of each run prints which flags were compiled in.

```sh
cargo build --release --locked --target-dir target/baseline
cargo build --release --locked --target-dir target/inline \
  --features dashmap-inline-more,clashmap-inline

for backend in dashmap clashmap; do
  for case in mixed scan churn; do
    case "$case" in
      mixed) ops=4000000 ;;
      scan)  ops=1024 ;;
      churn) ops=1000000 ;;
    esac
    for trial in 1 2 3 4 5 6 7; do
      case "$((trial % 2))" in
        1) set -- baseline inline ;;
        0) set -- inline baseline ;;
      esac
      for mode do
        "./target/$mode/release/cache-map-profile" "$backend" "$case" 16384 "$ops" 4
      done
    done
  done
done
```

The loop alternates build order. Compare medians and spread for
`slowest_reader_ms`, `scan_ms`, and `churn_ms`.

The local comparison used Rust 1.100.0 on a Ryzen 9 9950X3D, with seven runs
per build and workload. These are synthetic timings, not request latencies.

| Backend | Workload | Baseline median | Inline median | Change in elapsed time |
| --- | --- | ---: | ---: | ---: |
| DashMap | `mixed` | 181.63 ms | 177.83 ms | −2.1% |
| DashMap | `scan` | 336.64 ms | 347.73 ms | +3.3% |
| DashMap | `churn` | 391.55 ms | 375.20 ms | −4.2% |
| ClashMap | `mixed` | 188.40 ms | 188.12 ms | −0.1% |
| ClashMap | `scan` | 223.38 ms | 223.29 ms | ≈0% |
| ClashMap | `churn` | 393.94 ms | 384.57 ms | −2.4% |

DashMap scans were slower with inlining in all seven pairs. The churn
medians improved for both maps, but their timing ranges overlapped, as
did the mixed workloads. This is not a clear win for either flag.

## SCC and SIMD

`scc-map` uses `scc::HashMap` with synchronous upserts and bucket-locked
reads. `scc-index` uses `scc::HashIndex` with lock-free `peek_with` lookups
and guarded iteration; replacing an occupied entry uses `Entry::update`.
Both adapters are for SCC 3.9.1.

On Linux x86-64, build the portable SSE2 variant and an AVX2 variant in
separate directories. Only build/run the second command on an AVX2-capable
host (check `grep -qw avx2 /proc/cpuinfo`). Keep the same CPU baseline in
both builds so extra `target-cpu=native` features don't confound the comparison.
Build all backends with the same flags when comparing different maps.

```sh
RUSTFLAGS='-C target-cpu=x86-64' \
  cargo build --release --locked --target-dir target/sse2
RUSTFLAGS='-C target-cpu=x86-64 -C target-feature=+avx2' \
  cargo build --release --locked --target-dir target/avx2

CACHE_MAP_PROFILE_NO_PAUSE=1 ./target/sse2/release/cache-map-profile scc-map mixed 16384 4000000 4
CACHE_MAP_PROFILE_NO_PAUSE=1 ./target/avx2/release/cache-map-profile scc-map mixed 16384 4000000 4
```

The first log line reports `scc_simd=sse2` or `scc_simd=avx2` from the
compile-time target flags. Disassembly confirmed `pcmpeqb` on XMM registers
in the SSE2 build, and `vpcmpeqb` plus `vpmovmskb` on YMM registers in the
AVX2 build, in SCC's actual bucket lookups for both maps. SIMD is used
to match partial hashes inside buckets, not to vectorize full-map iteration.
The AVX2 build is benchmark-only and requires a compatible CPU to run.

```sh
objdump -Cd --no-show-raw-insn target/avx2/release/cache-map-profile |
  awk '/^[[:xdigit:]]+ </ { fn=$0 }
       /vpcmpeqb.*%ymm|vpmovmskb.*%ymm/ && fn ~ /scc::/ { print fn; print }'
```

### Local results

Seven runs in shuffled order without Heaptrack on the same Ryzen 9 9950X3D,
with 16,384 rows. All non-SCC entries below use the same AVX2 build and
default crate features. Times are medians in milliseconds.

| Backend / build | Mixed: slowest reader | Mixed: writer | 1,024 full scans | 1M churn updates |
| --- | ---: | ---: | ---: | ---: |
| DashMap / AVX2 | 133.53 | 80.58 | 208.95 | 364.93 |
| ClashMap / AVX2 | 179.28 | 113.75 | 217.12 | 373.38 |
| Papaya / AVX2 | 252.56 | 291.07 | 85.72 | 529.22 |
| SCC HashMap / SSE2 | 140.93 | 73.96 | 54.29 | 356.28 |
| SCC HashMap / AVX2 | 135.37 | 71.77 | 52.82 | 335.79 |
| SCC HashIndex / SSE2 | 130.95 | 140.17 | 56.22 | 673.15 |
| SCC HashIndex / AVX2 | 134.42 | 142.23 | 56.23 | 675.77 |

AVX2 reduced the SCC HashMap mixed-reader median by 4% and churn median
by 6%, but both timing ranges overlapped. It did not improve HashIndex
in this run. SCC HashMap's biggest advantage over DashMap was full scans;
mixed-reader times were close and the SCC writer was faster in this sample.

Heaptrack peak heap (one capture per cell, AVX2 builds, default operation
counts; `M` as reported by Heaptrack):

| Case | DashMap | ClashMap | Papaya | SCC HashMap | SCC HashIndex |
| --- | ---: | ---: | ---: | ---: | ---: |
| `load` | 9.08M | 9.15M | 7.31M | 11.23M | 11.33M |
| `mixed` | 9.21M | 9.24M | 7.34M | 11.21M | 11.27M |
| `scan` | 9.15M | 9.07M | 7.31M | 11.19M | 11.37M |
| `churn` | 9.19M | 9.23M | 7.49M | 11.22M | 16.92M |
| `pinned` | 9.15M | 9.19M | 51.10M | 11.19M | 47.90M |

HashMap cost about 2 MB more peak heap than DashMap in `mixed`, though
their RSS at `mixed_complete` under Heaptrack was close (15.5–15.7 MiB).
HashIndex kept more heap under ordinary churn and its peak rose with a
long guard. SCC HashMap is the stronger candidate if scans matter, not
an obvious choice for reducing heap. None of these results measure
production request latency or server RSS.

### First production migration

The server uses SCC HashMap for buy/sell orders, trade orders, and the
claim-tile cache. Rayon startup imports use synchronous upserts; live
workers and HTTP handlers use SCC's async operations. The scans still
walk the relevant caches and suppress duplicate keys if a concurrent
resize revisits them. No server-wide CPU flags changed.

The remaining DashMap/DashSet caches are unchanged. The standalone
benchmark does not include the per-route filtering, grouping, and
serialization work, so compare endpoint duration and server RSS before
extending the migration to more caches.
