# Native small-gzip replay, f22bb3201

**Keep the opt-in SDK patch.** Wire decode through callback delivery took
**12.60% less thread CPU** on the complete mixed capture and **21.88% less**
on gzip transactions. The candidate won all ten CPU pairs in both workloads.
The median full pass fell from **306.18 to 267.27 ms**; the gzip-transaction
pass fell from **177.76 to 139.94 ms**. This is an SDK replay result, not a
measurement of service-wide CPU.

| Workload / metric | Baseline median | Candidate median | Median paired change |
|---|---:|---:|---:|
| Mixed, CPU ms / 1,000 frames | 2.45647 | 2.14426 | −12.60% |
| Mixed, instructions / process (billions) | 18.477 | 15.040 | −18.60% |
| Mixed, cycles / process (billions) | 6.589 | 5.755 | −12.61% |
| Mixed, burst p99 / service p99 (µs) | 111.08 / 14.30 | 98.74 / 11.96 | −12.03% / −17.02% |
| Mixed, peak RSS in CPU run (MiB) | 216.16 | 216.23 | +0.01% |
| Gzip tx, CPU ms / 1,000 frames | 8.66240 | 6.81923 | −21.88% |
| Gzip tx, instructions / process (billions) | 10.152 | 6.700 | −34.00% |
| Gzip tx, cycles / process (billions) | 3.831 | 3.010 | −21.79% |
| Gzip tx, burst p99 / service p99 (µs) | 356.48 / 30.56 | 293.55 / 29.31 | −15.53% / −2.37% |
| Gzip tx, peak RSS in CPU run (MiB) | 79.42 | 79.45 | +0.06% |

Both builds produced the same full ordered transcript for **124,644 frames /
384,303 callbacks**, and for **20,521 gzip transactions / 138,611 callbacks**.
The full SHA-256 digests and all 80 timing samples are in [samples.jsonl](samples.jsonl).
Mixed burst and service p99 were lower in 9/10 pairs; gzip burst p99 in 10/10
and service p99 in 8/10. System: Ryzen 9 9950X3D, Linux x86-64, rustc 1.100.0.

## What is compared

[SDK patch](small-gzip.patch) against `f22bb32014d3377218375b6a033d3d74775027c9`.
Both binaries use that SDK, the same cacheless generated bindings (`mod.rs`
SHA-256 `b68926f6f27e6dcfff16d67294b053af09eb1c3a05cde0303697c1b15cf80bc6`),
the API's flate2 1.1.11 / zlib-rs 0.6.8, mimalloc v3, fat LTO and release
opt-level 3. Only the candidate enables `spacetimedb-sdk/native-small-gzip`.
It implies `streaming-decompression`; SDK defaults do not change. Native builds
with this feature compile bundled libdeflate via `cc`, without requiring a
system libdeflate. On wasm32, Cargo does not select the C dependency and the
browser socket keeps the Rust path.

The [driver](main.rs) visits **all 124,644 region-14 frames in wire order**:
one identity, six gzip snapshots, 104,116 uncompressed transactions and 20,521
gzip transactions. A second workload selects all gzip transactions in order,
including the five above the SDK's small-wire cutoff. The fixture reconstructs
gzip from the original private capture; see [sdk-decode](../sdk-decode/README.md).
The benchmark runs from resident, tagged wire bytes through SDK envelope decoding,
generated typed parsing, `process_message`, and callback delivery. It registers
insert/delete/update listeners for all observed live tables (insert only on the
event table) and initial-row listeners for all observed snapshot tables. The
separate parity pass interleaves length-framed SHA-256 inputs for every decoded
envelope and callback, including kind, table, both sides of an update, and every
ordered initial row. It emits counts and digests only.

CPU is thread CPU per 1,000 selected frames over four full passes after one
warmup. Perf counts user instructions/cycles for the entire process, including
startup and warmup. A separate warm run measures wall time to the last callback
per frame and from the start of a burst of 32 ready frames. Only frames with a
callback contribute to the percentiles. Peak RSS includes the file, indices,
allocator, decoded snapshots/rows and the runtime. No networking, scheduling
across the SDK's usual queues, app business logic, capture writing or SQL is
timed. Samples use alternating adjacent fresh processes pinned to CPU 5.

## Why the fast path is narrower than the inflate-only probe

* Reuse one lazily allocated native decompressor per native message loop. It
  retains no frame or output. Try only a gzip wire frame of at most 4,096 bytes,
  with no gzip flags and a single final dynamic block. A final four-byte size
  hint can only **skip** the attempt; it never changes the result or sizes a
  buffer. Native output has a fixed 8,191-byte stack limit. Exactly 8,192
  already used the streaming BSATN decoder on the baseline; its error details
  must remain the same. No global cache, output-sized allocation or new size
  limit. If an apparent small member spills, replay from the start through the
  existing Rust small/streaming path.
* libdeflate validates the first member's data CRC and size and ignores trailing
  data, but does **not** check optional FHCRC and is intentionally more
  permissive than zlib on DEFLATE Huffman codes. Even a *successful* native
  inflate with the right CRC isn't sufficient. The guard checks the bounded
  tree description: permitted alphabet sizes, and complete code spaces for the
  precode, literal/length and distance alphabets. Fixed blocks, multiblock
  streams, optional fields, incomplete codes (even valid singletons), suspect
  headers and every native error take the original path. Only after validation
  does the candidate use the baseline's slice BSATN decoder.
* The patch includes differential unit tests for native-success/SDK-error
  counterexamples (bad FHCRC, reserved HDIST, wrong bit in a singleton distance
  code), valid singleton fallback, every truncation and one-bit mutation of a
  small transaction, concatenated members/trailing junk, scratch and wire
  boundaries, corrupted trailers, malformed BSATN, and subsequent reuse.

The [coverage pass](coverage.json) hit the fast path on 19,982 of 20,516
small gzip transactions. All 534 remaining small frames had an oversize hint.
All six snapshots and five large gzip transactions bypassed it. On x86-64 the
[native allocator probe](native-memory.json) measured **11,568 requested heap
bytes in one allocation**, zero additional allocations over 1,000 inflates,
and one free. There is no retained output buffer. The `text` column of GNU
`size` increased by 42,981 bytes for this replay executable.

[Checks](checks.json): 39 SDK unit tests passed cacheless and 41 with client
cache enabled, plus all nine streaming integration tests in both configurations.
The unchanged explicit microbenchmark is ignored. With both cache and optimization
off, 37 passed. The wasm32 browser target compiled with the feature on; its
normal/build graph contains no libdeflate. A [two-connection socket
test](loopback.py) sent truncated and bad-CRC transactions before a valid
transaction, first-member trailing bytes, and a reducer reply with >8 KiB of
plaintext but <4 KiB of wire. The ordered initial/live/reducer/unsubscribe/
disconnect callback transcript matched on both connections.

**Integration:** add `native-small-gzip` to the workspace `spacetimedb-sdk`
dependency in `rust/api-server/Cargo.toml` if adopting the patch. At the native binary-message receive site retain `bytes`
for the existing capture callback. `SmallGzip::try_decode(&bytes)` returns
`None` for uncompressed frames; if composing with owned row-slice decoding,
invoke the owned baseline/sibling path in that fallback. The production patch
does not change the BSATN decoder, generated bindings, or the SDK's public API.

## Reproduce

Requires a private `region14.wire` (SHA-256
`c5c7878915085400621d32396d72a8d93c37ca80e6e2fc13859a3613a3bb3997`), a
fresh `$out`, matching generated bindings, a C compiler, `perf`, Bun
and `taskset`. The SDK's own `Cargo.lock` pins flate2 1.1.4 / zlib-rs 0.5.5;
the callback replay intentionally uses the API lock instead. Cache
`libdeflater` and `libdeflate-sys` 1.26.1 before the offline driver build.

```sh
python3 build.py prepare "$out" "$SDK_REPO"
python3 build.py base "$out"
python3 build.py candidate "$out"
"$out/target/release/coverage" "$WIRE"
python3 loopback.py "$out"
python3 measure.py "$out" "$WIRE" 5 10 > "$out/samples.jsonl"
python3 summary.py < "$out/samples.jsonl"
```

`measure.py` holds `/tmp/opencode/cpu5.lock` across the run (override with
`BENCH_CPU_LOCK` when coordinating other CPU-5 workloads). To check requested C
heap bytes separately, compile [native-memory.c](native-memory.c) with the
bundled build's `-I<OUT_DIR>/include <OUT_DIR>/lib/libdeflate.a`. It counts
allocator calls across construction, 1,000 inflates and drop.

For the independent SDK tests, apply the patch in a fresh checkout of
`f22bb3201` and run:

```sh
cargo test --locked -p spacetimedb-sdk --lib --test streaming_decompression --features native-small-gzip
cargo test --locked -p spacetimedb-sdk --lib --test streaming_decompression --no-default-features --features native-small-gzip
cargo test --locked -p spacetimedb-sdk --lib --no-default-features
cargo check --locked -p spacetimedb-sdk --target wasm32-unknown-unknown --no-default-features --features browser,native-small-gzip
```

On this host the SDK workspace tests use its pinned Rust 1.93.0, OpenSSL 3
(`OPENSSL_LIB_DIR=/usr/lib/openssl-3`, `OPENSSL_INCLUDE_DIR=/usr/include/openssl-3`)
and `RUSTFLAGS="--cfg tokio_unstable -C link-arg=-fuse-ld=/usr/bin/mold"` so
the local Cargo linker configuration doesn't suppress `tokio_unstable`.
