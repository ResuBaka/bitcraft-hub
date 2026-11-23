# Inventory: omit the unused old row

Inventory callbacks now clone only the row the worker consumes. On the private
region-14 capture this saved **20.8% of delivery CPU** (12/12 alternating-order
pairs) and **one allocation per update**. Insert and remove still deliver their
complete row. The fixed-size queue slot remains 128 bytes; the saving is in
the old row’s heap-owned pockets.

| Metric | Both rows | New row only |
|---|---:|---:|
| Median thread CPU, ns / inventory callback | 128.56 | 101.84 |
| Allocation calls / 131 capture passes | 496,753 | 250,473 |
| Requested bytes / 131 passes | 239,539,200 | 122,691,392 |

The replay decoded 104,116 uncompressed transactions outside timing and paired
inventory rows by primary key within each transaction. The resulting stream
had 11 inserts, 21 removes, and 1,880 updates with 31,856 old pockets. The
allocation difference is 1,880 calls and 891,968 requested bytes per pass:
exactly one allocation per old pocket vector and 28 bytes per old pocket.
The parity pass checked every retained row (including all pocket fields),
operation kind, region, and FIFO position against the decoded stream.

**Boundary:** Start with borrowed inventory rows, project the actual application
message, enqueue on the API’s Tokio-backed worker channel in bursts of 32,
receive each message, select the row as the worker does, and drop the payload.
This single-thread ready-queue test excludes SDK dispatch, listener/worker
metric increments, scheduling between OS threads, DB writes, and notifications.
It does not measure end-to-end API CPU or queue latency. Separate worker tests
cover the DB operations and outbound events.

Both arms ran in the same release API lib-test binary on CPU 2 (Ryzen 9
9950X3D), with the system allocator, a full parity/warmup pass per arm, then
12 AB/BA pairs of 250,472 messages each using the thread CPU clock. A separate
instrumented binary counted thread-local allocation, zeroed-allocation and
reallocation requests with identical input; its timings are excluded.
[Raw samples](samples.jsonl) contain no capture data.

## Reproduce on Linux

From `rust/api-server`, set `INVENTORY_WIRE` to the private region-14 wire file
(SHA-256 `c5c7878915085400621d32396d72a8d93c37ca80e6e2fc13859a3613a3bb3997`).
If the temporary file has expired, the SDK heap replay’s `prepare` command
can reconstruct it from the original private `STDBCAP1` recording.

```sh
features=tokio-worker-queues,spacetimedb-tokio-websockets-rustls
export CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_DEBUG=0
for mode in "$features" "$features,bench"; do
  taskset -c 2 cargo test --offline --locked --release -p api --lib \
    --features "$mode" inventory_new_row_delivery -- --ignored --nocapture
done
```

`inventory::bitcraft::initial_reconciliation_tests` also checks that snapshot
projection keeps both the outer allocation and each pocket buffer, that whole
and chunked snapshots reconcile before the following live update, and that
shutdown drains accepted input before rejecting a late send. Mixed live
events exercise notification order, latest-by-key persistence, and changelog
ordering through the projected queue.
