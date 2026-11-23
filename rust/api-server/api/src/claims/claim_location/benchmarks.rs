//! Ignored profiling tests for the claim-location index. Run one at a time:
//!
//! ```sh
//! cargo test -p api --lib claims::claim_location::benchmarks -- --ignored --nocapture
//! ```
//!
//! `profile_claim_index_retained_heap` prints the index's retained heap
//! (sorted arrays + claim dictionary) and bytes per tile. The latency tests
//! print per-operation nanoseconds. Raw outputs are saved per implementation
//! under `/tmp/opencode/claim-location-compact-2026-11-22/`.

use super::{ClaimAtLocation, MAX_TILE_EDITS, OverworldTile, SortedOverworld, TileRow};
use std::hint::black_box;
use std::time::Instant;

const BENCH_TILES: usize = 3_291_000;
const BENCH_EDITS: usize = 4_096;

impl super::ClaimLocationSubscriptions {
    pub(crate) fn disable_search_hints_for_replay(&self, region: entity::shared::Region) {
        self.update(region, |state| {
            let super::Overworld::Ready(index) = &mut state.overworld else {
                panic!("replay needs a ready claim index")
            };
            assert!(!index.base.search_hints.is_empty());
            index.base.search_hints = Box::default();
        });
    }
}

/// Deterministic xorshift so baseline and instrumented runs build identical data.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

fn generate_rows(count: usize) -> (Vec<TileRow>, Vec<u64>) {
    let mut rng = Rng::new(0x9E37_79B9_7F4A_7C15);
    let mut rows = Vec::with_capacity(count);
    let mut ids = Vec::with_capacity(count);
    const CLAIMS_PER_TILE: u64 = 32;
    for i in 0..count as u64 {
        // Unique pseudo-random IDs over the full u64 range.
        let id = i.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        // One claim covers ~CLAIMS_PER_TILE tiles; claim IDs repeat.
        let claim = (i / CLAIMS_PER_TILE)
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let x = ((rng.next_u64() >> 32) % 2048) as i32 - 1024;
        let z = ((rng.next_u64() >> 48) % 2048) as i32 - 1024;
        rows.push(TileRow {
            id,
            tile: OverworldTile {
                claim,
                coords: (x, z),
            },
        });
        ids.push(id);
    }
    (rows, ids)
}

fn claim_value(result: ClaimAtLocation) -> u64 {
    match result {
        ClaimAtLocation::Claim(claim) => claim,
        ClaimAtLocation::Unclaimed => 0,
        ClaimAtLocation::Unknown => 1,
    }
}

fn read_vm_hwm_kb() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find(|line| line.starts_with("VmHWM"))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse().ok())
}

#[test]
#[ignore = "profile: claim index — retained heap and latency"]
fn profile_claim_index_retained_heap() {
    let (rows, _ids) = generate_rows(BENCH_TILES);
    let index = SortedOverworld::new(rows);
    let rows = index.rows();
    let bytes = index.bytes();
    eprintln!(
        "tiles={rows} retained_bytes={bytes} bytes_per_tile={:.2}",
        bytes as f64 / rows as f64
    );
}

#[test]
#[ignore = "profile: claim index — lookup latency (hits and misses)"]
fn profile_claim_index_lookup_latency() {
    let (rows, _ids) = generate_rows(BENCH_TILES);
    let hits: Vec<(i32, i32)> = rows.iter().map(|row| row.tile.coords).collect();
    let index = SortedOverworld::new(rows);

    // Warm one pass so page faults are not part of the measured path.
    let mut sum = 0u64;
    for &coord in hits.iter().take(1_000_000) {
        sum ^= claim_value(index.lookup(coord));
    }
    black_box(sum);

    const ITERS: usize = 4_000_000;
    let mut sum = 0u64;
    let start = Instant::now();
    for i in 0..ITERS {
        sum ^= claim_value(index.lookup(hits[i % hits.len()]));
    }
    black_box(sum);
    let hit_ns = start.elapsed().as_nanos() as f64 / ITERS as f64;

    let mut sum = 0u64;
    let start = Instant::now();
    for i in 0..ITERS {
        let coord = (hits[i % hits.len()].0 + 4096, hits[i % hits.len()].1);
        sum ^= claim_value(index.lookup(coord));
    }
    black_box(sum);
    let miss_ns = start.elapsed().as_nanos() as f64 / ITERS as f64;

    eprintln!("hit_ns_per_lookup={hit_ns:.1} miss_ns_per_lookup={miss_ns:.1}");
}

#[test]
#[ignore = "profile: claim index — edit latency"]
fn profile_claim_index_edit_latency() {
    let (rows, ids) = generate_rows(BENCH_TILES);
    let mut index = SortedOverworld::new(rows);
    let mut rng = Rng::new(987_654_321);
    let start = Instant::now();
    for i in 0..BENCH_EDITS {
        let id = ids[(rng.next_u64() as usize) % ids.len()];
        let Some(previous) = index.get(id) else {
            continue;
        };
        let wide = i % 2 == 0;
        let next = OverworldTile {
            claim: previous.claim,
            coords: if wide {
                (previous.coords.0.wrapping_add(300_000), previous.coords.1)
            } else {
                (previous.coords.0.wrapping_add(1), previous.coords.1)
            },
        };
        index.replace(id, Some(previous), Some(next));
        black_box(&index);
    }
    let edit_ns = start.elapsed().as_nanos() as f64 / BENCH_EDITS as f64;
    eprintln!("edit_ns_per_replace={edit_ns:.1}");
}

#[test]
#[ignore = "profile: claim index — rebuild time and retained heap"]
fn profile_claim_index_rebuild() {
    let (rows, ids) = generate_rows(BENCH_TILES);
    let mut index = SortedOverworld::new(rows);
    let before = index.bytes();
    let mut rng = Rng::new(987_654_321);
    for i in 0..MAX_TILE_EDITS {
        let id = ids[(rng.next_u64() as usize) % ids.len()];
        let Some(previous) = index.get(id) else {
            continue;
        };
        let next = OverworldTile {
            claim: previous.claim,
            coords: (previous.coords.0.wrapping_add(1), previous.coords.1),
        };
        index.replace(id, Some(previous), Some(next));
    }
    let start = Instant::now();
    index.rebuild();
    let rebuild_ms = start.elapsed().as_millis();
    let after = index.bytes();
    eprintln!("rebuild_ms={rebuild_ms} retained_before={before} retained_after={after}");
}

#[test]
#[ignore = "profile: claim index — rebuild under heaptrack"]
fn profile_claim_index_rebuild_heaptrack() {
    let (rows, ids) = generate_rows(BENCH_TILES);
    let mut index = SortedOverworld::new(rows);
    eprintln!("MARKER: retained_before_rebuild bytes={}", index.bytes());
    let mut rng = Rng::new(987_654_321);
    for i in 0..MAX_TILE_EDITS {
        let id = ids[(rng.next_u64() as usize) % ids.len()];
        let Some(previous) = index.get(id) else {
            continue;
        };
        let next = OverworldTile {
            claim: previous.claim,
            coords: (previous.coords.0.wrapping_add(1), previous.coords.1),
        };
        index.replace(id, Some(previous), Some(next));
    }
    eprintln!(
        "MARKER: before_rebuild edits={} dirty_coords={} VmHWM_kb={:?}",
        index.edits.len(),
        index.dirty_coords.len(),
        read_vm_hwm_kb()
    );
    let start = Instant::now();
    index.rebuild();
    eprintln!(
        "MARKER: after_rebuild bytes={} elapsed_ms={} VmHWM_kb={:?}",
        index.bytes(),
        start.elapsed().as_millis(),
        read_vm_hwm_kb()
    );
    black_box(&index);
}
