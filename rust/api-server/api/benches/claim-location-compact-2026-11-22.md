# Claim-location index compaction — 20-byte compact base

Measured and implemented starting from the profile's measured coordinate-sorted
20-byte/tile prototype. This is the claim-index priority of the 2026-11-22
memory profile (section 3). The benchmark puts 3,291,000 tiles in one index;
the earlier server profile had roughly that many tiles across nine regions.

Artifacts: benchmark harness `rust/api-server/api/src/claims/claim_location/benchmarks.rs`
(ignored tests, run via
`cargo test -p api --lib claims::claim_location::benchmarks -- --ignored --nocapture`),
raw outputs under `/tmp/opencode/claim-location-compact-2026-11-22/`
(`new_*` / `old_*`), implementation changes in
`rust/api-server/api/src/claims/claim_location.rs`.

## Summary of actual gains (measured)

| Metric (3,291,000 tiles, synthetic, test profile) | Current 36 B/tile | New 20.25 B/tile | Change |
| --- | ---: | ---: | --- |
| Retained base arrays and dictionary | 118,476,000 B | 66,642,752 B | **−49.43 MiB (−43.7%)** |
| Bytes/tile | 36.00 | 20.25 | −15.75 B/tile |
| Lookup, present coordinate (ns) | 436.1 | 192.5 | −55.9% |
| Lookup, absent coordinate (ns) | 28.0 | 29.8 | ≈ same |
| Edit (`get` + `replace`, ns) | 3,276.9 | 3,242–3,616 (median ~3,400) | +4% at median |
| Rebuild time | 131 ms | 622–679 ms (median ~660) | ~5× slower |
| Rebuild retained heap | 118,476,000 → 118,476,000 | 66,642,752 → 66,642,752 | size-preserving |
| Heaptrack peak live heap, entire build/edit/rebuild test | 304.03 MB | 331.04 MB | **+27.01 MB** |
| Process VmHWM before → after rebuild (KiB, under Heaptrack) | 205,476 → 311,180 | 270,044 → 337,964 | higher final high-water mark |

Extrapolated impact: the profile attributed **115.9 MiB (48%)** of the 239.1 MiB
late-heap sample to this index. The synthetic saving is about **21% of that
live-heap sample**. Actual server savings depend on the number of tiles that
fit signed 16-bit axes and the number of distinct claims per region.

## Layout implemented (coordinate-sorted, 20 bytes/tile + dictionary)

`CompactBase` stores, in coordinate order:

- `coords: Box<[u32]>` — packed coordinates: two's-complement 16-bit axes
  (`pack` returns `None` outside `[-32768, 32767]`), so negatives are exact and
  there is one total order over the sorted arrays.
- `ids: Box<[u64]>` — entity IDs, same coordinate order.
- `claim_slots: Box<[u32]>` — claim-dictionary slot per tile (4 B).
- `perm: Box<[u32]>` — in ID order: `perm[j]` is the coordinate position of the
  j-th smallest ID; `ids[perm[j]]` is the sorted ID sequence.

Plus the claim dictionary `claims: Box<[u64]>` (slot → claim ID, one entry per
distinct claim per region — claim IDs repeat across tiles) and the wide
fallback (`wide_by_id: HashMap<u64, OverworldTile>`, `wide_by_coord:
HashMap<TileCoordinates, Vec<u64>>`) for out-of-range coordinates.

Total per-tile arrays: 4 + 8 + 4 + 4 = 20 B/tile plus 8 B per distinct claim
dictionary entry. `LiveRegion`, `ClaimLocationSubscriptions`, the edit overlay
(`edits`), `dirty_coords`, `mobile_tile`, `ClaimLocationSnapshot` and all public
call sites are unchanged.

## Benchmark method

- Data: deterministic xorshift; 3,291,000 tiles; pseudo-random unique u64 IDs;
  ~32 tiles per claim; coordinates in `[-1024, 1024)²`; a fraction of
  coordinates collide (overlapping claims, mixed → Unknown).
- Lookup: 4,000,000 iterations; "hit" queries at coordinates that are in the
  index; "miss" queries perturbed by +4096 in x (absent). Times are run
  averages.
- Edit: 4,096 `replace` calls on random IDs; half move to an in-range
  coordinate, half to a wide coordinate.
- Rebuild: 4,096 edits, then `rebuild()`; repeated 3×; also run under Heaptrack
  with `VmHWM` markers.
- Baseline run: the original `claim_location.rs` from `9d89101e` plus compatibility
  shims appended (`rows()`/`bytes()` = `by_id.len()*24 + coords.len()*8 +
  rows_by_coord.len()*4`), so the same benchmark module ran against both
  implementations.

Note: these are in the crate's test profile (opt-level 1 + debuginfo), not
release; old edit/rebuild are single-run baselines on a noisy host. Whole-server
Heaptrack is not claimed here; no scratch databases were needed (index-only
workload). No whole-server profiling is in scope for this priority.

## Semantics preserved (tests, 16 passing; full `api` lib suite: 403 passing)

- Existing tests updated to the new internals and passing; new tests added:
  - `pack_unpack_preserves_narrow_and_wide_coordinates` — boundaries
    `±32767/±32768`, negative axes, round-trips; out-of-range → fallback.
  - `wide_coordinates_use_fallback_for_lookups_and_edits` — wide tiles through
    the fast path, wide→narrow, narrow→wide, deletion; lookups at
    out-of-range coordinates.
  - `wide_and_negative_coordinates_survive_rebuild` — negative coordinates,
    `±i16` boundaries, wide outliers, `get` and `lookup` identical after
    `rebuild`.
  - `dirty_wide_and_narrow_occupants_mix_to_unknown` — dirty lists include
    wide occupants; mixed occupants resolve to Unknown; deleting the narrow
    one restores the wide claim.
  - `live_mutations_match_rebuilt_snapshot_with_wide_and_negative_coordinates`
    — 2,000 random claim/location ops with wide/negative coordinates, periodic
    forced rebuilds, cross-checked against `ClaimLocationSnapshot` per step.
- Overlapping claims, ambiguity, moves between overworld/interior, reconnect
  (`reset`/`invalidate`) and initial-join semantics are unchanged — the overlay
  (`edits`, `dirty_coords`) and `ClaimLocationSubscriptions` are untouched.

## Notes

- Rebuild is ~5× slower than before (~0.65 s vs ~0.13 s in the test profile).
  Reaching 4,096 distinct edits or 8,192 dirty coordinates triggers a rebuild
  under the region write lock. Lookups in that region can wait for it; these
  single-thread timings do not measure reader wait time or rebuild frequency.
- `bytes()` counts array and dictionary payloads and estimates wide-map storage;
  it excludes the overlay. This is a benchmark helper, not an exported metric.
- `clippy` clean on changed code (added `#[allow(clippy::large_enum_variant)]`
  on the unchanged `Overworld` enum, whose `Ready` variant is now larger).
