# Receive CPU after TLS: region-14 SDK decode

**The SDK can keep the wire format and save ~10% of client CPU from the
compression tag through generated row parsing on this replay.** The
[tested SDK patch](small-gzip.patch) is against `d83d75e8`. It uses flate2's
`bufread::GzDecoder` over the existing input slice and tries decoding small
gzip messages into an 8 KiB stack buffer. On verified EOF it uses the SDK's
regular slice BSATN decoder. If the output fills the buffer, the SDK streams
the prefix plus the remainder through its existing decoder: no second inflate,
size limit or unbounded decompressed staging vector. Other compression tags
take their existing paths. The API now uses the patch via SDK commit
[`bdf9794b9`](https://github.com/ResuBaka/SpacetimeDB/commit/bdf9794b9429a58014bb73ae6db4e439d64d3e55).

## Same-wire comparisons

Same-binary matched process runs, pinned to CPU 11 (Ryzen 9 9950X3D).
Values are median **client thread CPU ms for one complete selected pass**
after one warmup. Actual generated BitCraft `DbUpdate` parsing and destruction
are included in the last two columns. No sockets, queues, callback work,
database writes or compression of the input during timing.

| First search: five rotated order blocks | 20,521 gzip transaction envelopes | Six snapshots through rows | All 124,644 messages through rows |
|---|---:|---:|---:|
| Stock SDK | 148.4 | 49.2 | 267.1 |
| Use the compressed slice directly as gzip input | 142.0 | 47.6 | 259.0 |
| Also reset a connection-owned gzip decoder | 139.4 | 47.9 | 257.6 |
| Also reuse 8 KiB of scratch for small messages | 122.0 | 47.5 | 236.9 |
| Uncompressed input (changes wire bytes) | 13.0 | 18.9 | 95.4 |

**Simplification experiment.** In a second run cycling all six permutations
of stock, stateless scratch, and reset + scratch, the stateless variant used
**240.7 vs 267.3 ms** through all rows and **125.5 vs 149.1 ms** for the
gzip transaction envelopes, winning all six comparisons in both cases.
Snapshots were nearly unchanged (48.4 vs 49.3 ms). Retaining the inflater
and scratch for the connection got the full pass down to 237.4 ms. The
stateless patch captures most of the benefit without requiring the newer
decoder reset API in the SDK workspace. The timings compare the candidates
via the API pin's real streaming/slice decoders; the SDK patch was separately
compiled and tested in the fork.

**A deployable bandwidth trade.** Requesting `Compression::None` removes
the inflate work, at **116,823,549 vs 61,615,319 payload bytes** for the
same messages. Snapshots alone expand from ~8.1 MB to ~44.0 MB. This is
not a prediction of elapsed startup time: network transfer and TLS process
the extra bytes. Test `BITCRAFT_HUB_API__SPACETIMEDB__COMPRESSION=none` on
a bandwidth-rich deployment and compare CPU, last-region-ready and network
bytes before changing the default.

## Checks and reproduction

The runner validates reserialized server messages against the stock decoder
for every captured frame, rejects truncated/bad gzip trailers and decodes a
valid frame immediately afterward. The SDK's streaming tests also cover
trailing bytes/members, invalid BSATN, corrupt trailers after parsing, and
highly compressible fields straddling 8191/8192/8193 bytes. The capture has
104,116 uncompressed transactions; those keep using the existing path.

Use the private region-14 `STDBCAP1` capture reframed by the SDK's
`heap-replay prepare CAPTURE WIRE` helper. The original gzip bytes aren't
stored; in this fixture every reframed wire length matches the recorded
length. Reframed SHA-256:
`c5c7878915085400621d32396d72a8d93c37ca80e6e2fc13859a3613a3bb3997`.
Keep derived frames private. From this directory:

```sh
cargo build --release --locked
python3 measure.py target/release/sdk-decode "$WIRE" 11 > samples.jsonl
python3 measure.py target/release/sdk-decode "$WIRE" 11 --stateless > stateless.jsonl
```

The standalone benchmark lock keeps the pre-optimization SDK (`d83d75e8`)
as its control and uses the API's flate2/zlib-rs backend. `none` constructs
tag-0 frames from the identical plaintext before
warmup. In that arm “gzip transactions” denotes the same selected messages
that were compressed in the source, not gzip input to the timed decoder.
