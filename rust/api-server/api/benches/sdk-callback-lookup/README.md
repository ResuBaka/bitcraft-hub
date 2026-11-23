# One registry lookup per changed table

**Keep the change.** In 16 alternating release replay pairs pinned to CPU 3,
the candidate used **2.74% less thread CPU** per 1,000 transactions (median
paired change; lower in 15/16 pairs). Both builds delivered exactly 245,643
callbacks for 104,116 transactions, with ordered checksum `bb0c984258351b7a`.
Listener membership and loopback lifecycle checks passed on both builds with
and without client caching.

| Metric | Baseline median | Candidate median | Median paired change | Pairs lower |
|---|---:|---:|---:|---:|
| Thread CPU ms / 1,000 tx | 0.6760 | 0.6563 | −2.74% | 15/16 |
| User instructions, billions / process | 4.347 | 4.179 | −3.87% | 16/16 |
| User cycles, billions / process | 1.538 | 1.502 | −3.20% | 15/16 |
| Burst p99, µs | 30.04 | 30.18 | +2.74% | 7/16 |
| Service p99, µs | 1.83 | 1.73 | −0.25% | 8/16 |

Thread CPU covers four warm passes. `perf stat` covers the entire process;
latencies come from separate processes. The paired change compares adjacent
runs, not the two independent medians. There was no consistent p99 improvement.
These are SDK replay results, not whole-API measurements. [Raw pairs](samples.jsonl).

The regional Rust bindings used to ask `DbCallbacks::has_callbacks(name)` before
calling `invoke_table_row_callbacks(name, ...)`, which looked up the same table
again using `entry(...).or_default()`. The candidate generator emits just the
dispatch call. The [SDK](registry.patch) rejects unchanged diffs, then uses
`get_mut` once. Missing tables return immediately; tables whose last listener
was removed have empty callback maps. Owned-initial dispatch also uses a
non-inserting lookup. The existing `has_callbacks` method remains available for
previously generated clients.

## Checks and scope

- Baseline: SDK `bcd163b7c`, regional bindings at `a5344722`. Candidate: same
  builds with the registry change and regenerated public `schema7.json` bindings.
  All 329 dispatch arms retain the same table order. The build script checks
  that nothing else in the generated file changed besides its version comment.
- The [membership fixture](membership.rs) starts with no listener, removes one
  of two, removes the last, and registers again. It checks old/new rows, an
  update with no listeners, and a switch from update to delete callbacks.
  The [loopback lifecycle fixture](../sdk-owned-initial/lifecycle.rs) checks
  empty and populated initial snapshots, borrowed-before-owned delivery,
  cross-table ordering, reducer completion, unsubscribe and disconnect. Both
  fixtures run in cached and cacheless builds.
- Performance uses the [existing end-to-end SDK replay](../sdk-live/README.md).
  Both builds run the same uncompressed transactions from resident wire bytes
  through typed decoding and callbacks; the parity pass hashes event kind, table
  name and ordered row values. The timed path excludes networking, API handler
  bodies and database writes.

## Reproduce

Use the SDK checkout with the updated Rust generator, the crafting-list checkout,
Cargo, Bun, `perf`, and the private `region14.wire` recording. `$out` must be a
fresh directory and `$generated` an empty output directory outside the checkout.

```sh
cd "$SDK_REPO"
cargo build --offline --locked -p spacetimedb-cli --bin spacetimedb-cli
# Even for a JSON schema the CLI checks its default module directory exists.
work=$(mktemp -d)
mkdir "$work/spacetimedb"
(cd "$work" && "$SDK_REPO/target/debug/spacetimedb-cli" generate --no-config --lang rust \
  --module-def "$CRAFT/rust/api-server/schema7.json" --out-dir "$generated")
cd "$CRAFT/rust/api-server/api/benches/sdk-callback-lookup"
python3 build.py "$SDK_REPO" "$out" "$generated/mod.rs"
python3 ../sdk-owned-initial/measure.py "$out" "$WIRE" 3 > "$out/samples.jsonl"
python3 ../sdk-owned-initial/summary.py < "$out/samples.jsonl"
```
