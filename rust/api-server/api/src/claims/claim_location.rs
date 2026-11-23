//! Claim lookup primitives. An absent or incomplete snapshot must never be
//! interpreted as proof that a location is unclaimed.
#![allow(dead_code)] // Includes standalone snapshot helpers used by tests.

use entity::shared::Region;
use parking_lot::RwLock;
use std::collections::{HashMap, VecDeque};
use std::ops::Range;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClaimAtLocation {
    Claim(u64),
    Unclaimed,
    Unknown,
}

type TileCoordinates = (i32, i32);

// The index needs equality and a consistent sort order, not signed spatial
// ordering. Preserve the full bit pattern of both axes in one comparison key.
fn coordinate_key((x, z): TileCoordinates) -> u64 {
    (u64::from(x as u32) << 32) | u64::from(z as u32)
}

// Tiles whose coordinates fit two signed 16-bit axes use the compact base.
// Two's-complement packing keeps negative coordinates exact and gives the
// sorted arrays one consistent total order; anything else (other regions,
// out-of-range moves) is retained in the wide fallback.
fn pack(coords: TileCoordinates) -> Option<u32> {
    let (x, z) = coords;
    if i16::try_from(x).is_err() || i16::try_from(z).is_err() {
        return None;
    }
    Some((u32::from(x as u16) << 16) | u32::from(z as u16))
}

fn unpack(packed: u32) -> TileCoordinates {
    ((packed >> 16) as i16 as i32, packed as u16 as i16 as i32)
}
const MAX_UNMATCHED_LOCATIONS: usize = 4096;
const MAX_TILE_EDITS: usize = 4096;
const MAX_DIRTY_COORDS: usize = 2 * MAX_TILE_EDITS;

#[derive(Clone, Copy, PartialEq, Eq)]
enum TileLocation {
    Overworld(TileCoordinates),
    Interior,
}

impl From<(u32, i32, i32)> for TileLocation {
    fn from((dimension, x, z): (u32, i32, i32)) -> Self {
        if dimension == 1 {
            Self::Overworld((x, z))
        } else {
            Self::Interior
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct OverworldTile {
    claim: u64,
    coords: TileCoordinates,
}

#[derive(Clone, Copy)]
struct TileRow {
    id: u64,
    tile: OverworldTile,
}

/// Coordinate-sorted compact base for narrow tiles.
///
/// Per-tile arrays are 4 bytes of packed coordinate + 8 bytes of entity ID +
/// 4 bytes of claim-dictionary slot = 16 bytes, plus one 4-byte entry of the
/// ID-order permutation, so 20 bytes/tile. Claim IDs repeat across tiles; a
/// dense dictionary stores each distinct claim once per region.
struct CompactBase {
    /// Packed coordinates in coordinate order (ascending).
    coords: Box<[u32]>,
    /// Direct-mapped (packed coordinate, lower bound) pairs in `coords`.
    /// This array belongs to the immutable base, never to its edit overlay.
    search_hints: Box<[AtomicU64]>,
    /// Entity IDs aligned with `coords` in the same coordinate order.
    ids: Box<[u64]>,
    /// Claim-dictionary slot per tile, aligned with `coords`.
    claim_slots: Box<[u32]>,
    /// In ID order: `perm[j]` is the coordinate position of the j-th smallest ID.
    perm: Box<[u32]>,
    /// Claim dictionary: slot -> claim ID.
    claims: Box<[u64]>,
    /// Out-of-range tiles by ID.
    wide_by_id: HashMap<u64, OverworldTile>,
    /// Out-of-range tile IDs by coordinate.
    wide_by_coord: HashMap<TileCoordinates, Vec<u64>>,
}

impl CompactBase {
    const HINT_BITS: u32 = 9;

    fn build(tiles: Vec<(u64, OverworldTile)>) -> Self {
        let mut narrow: Vec<(u64, u64, u32)> = Vec::with_capacity(tiles.len());
        let mut wide_by_id = HashMap::new();
        let mut wide_by_coord: HashMap<TileCoordinates, Vec<u64>> = HashMap::new();
        for (id, tile) in tiles {
            match pack(tile.coords) {
                Some(key) => narrow.push((id, tile.claim, key)),
                None => {
                    wide_by_coord.entry(tile.coords).or_default().push(id);
                    wide_by_id.insert(id, tile);
                }
            }
        }
        narrow.sort_unstable_by_key(|&(_, _, key)| key);
        let count = u32::try_from(narrow.len()).expect("a region has too many overworld tiles");
        let ids: Vec<u64> = narrow.iter().map(|&(id, _, _)| id).collect();
        let coords: Vec<u32> = narrow.iter().map(|&(_, _, key)| key).collect();
        // Claim dictionary: one entry per distinct claim ID. `slot_of` is
        // built while walking `narrow`, so each tile costs one lookup.
        let mut claims: Vec<u64> = Vec::new();
        let mut slot_of: HashMap<u64, u32> = HashMap::new();
        let claim_slots: Vec<u32> = narrow
            .iter()
            .map(|&(_, claim, _)| {
                *slot_of.entry(claim).or_insert_with(|| {
                    let slot = u32::try_from(claims.len()).expect("too many claims");
                    claims.push(claim);
                    slot
                })
            })
            .collect();
        // `perm` is in ID order so coordinate searches stay contiguous; ID
        // searches binary-search `ids[perm[j]]`.
        let mut by_id: Vec<u32> = (0..count).collect();
        by_id.sort_unstable_by_key(|&pos| ids[pos as usize]);
        Self {
            search_hints: if coords.len() >= 4096 {
                // (0, 0) is valid even before the first search: zero is the minimum key.
                (0..1 << Self::HINT_BITS)
                    .map(|_| AtomicU64::new(0))
                    .collect()
            } else {
                Box::default()
            },
            coords: coords.into_boxed_slice(),
            ids: ids.into_boxed_slice(),
            claim_slots: claim_slots.into_boxed_slice(),
            perm: by_id.into_boxed_slice(),
            claims: claims.into_boxed_slice(),
            wide_by_id,
            wide_by_coord,
        }
    }

    /// Coordinate positions of the compact-base tiles at `coords`.
    fn base_positions_at(&self, coords: TileCoordinates) -> Range<usize> {
        let Some(key) = pack(coords) else {
            return 0..0;
        };
        let search = || self.coords.partition_point(|&packed| packed < key);
        let start = if self.search_hints.is_empty() {
            search()
        } else {
            let bucket = key.wrapping_mul(0x9e37_79b9) >> (32 - Self::HINT_BITS);
            let hint = &self.search_hints[bucket as usize];
            // Readers may replace each other's hints. An atomic pair prevents
            // mixing a different key's position into this lookup; only hits matter.
            let pair = hint.load(Ordering::Relaxed);
            if (pair >> 32) as u32 == key {
                pair as u32 as usize
            } else {
                let pos = search();
                hint.store((u64::from(key) << 32) | pos as u64, Ordering::Relaxed);
                pos
            }
        };
        let count = self.coords[start..]
            .iter()
            .take_while(move |&&packed| packed == key)
            .count();
        start..start + count
    }

    /// `(id, claim)` of the compact-base tiles at `coords`.
    fn base_at(&self, coords: TileCoordinates) -> impl Iterator<Item = (u64, u64)> + '_ {
        self.base_positions_at(coords)
            .map(|pos| (self.ids[pos], self.claims[self.claim_slots[pos] as usize]))
    }

    /// Tile for `id` in the compact base.
    fn get_by_id(&self, id: u64) -> Option<OverworldTile> {
        match self
            .perm
            .binary_search_by_key(&id, |&pos| self.ids[pos as usize])
        {
            Ok(index) => {
                let pos = self.perm[index] as usize;
                Some(OverworldTile {
                    claim: self.claims[self.claim_slots[pos] as usize],
                    coords: unpack(self.coords[pos]),
                })
            }
            Err(_) => self.wide_by_id.get(&id).copied(),
        }
    }

    /// All compact-base tiles in ID order.
    fn rows_by_id(&self) -> impl Iterator<Item = (u64, OverworldTile)> + '_ {
        self.perm.iter().map(|&pos| {
            let pos = pos as usize;
            (
                self.ids[pos],
                OverworldTile {
                    claim: self.claims[self.claim_slots[pos] as usize],
                    coords: unpack(self.coords[pos]),
                },
            )
        })
    }
}

struct SortedOverworld {
    base: CompactBase,
    edits: HashMap<u64, Option<OverworldTile>>,
    // An empty list masks a coordinate whose base occupants were all removed.
    dirty_coords: HashMap<TileCoordinates, Vec<(u64, u64)>>,
}

impl SortedOverworld {
    fn new(mut rows: Vec<TileRow>) -> Self {
        rows.sort_unstable_by_key(|row| row.id);
        let mut index = Self {
            base: CompactBase::build(Vec::new()),
            edits: HashMap::new(),
            dirty_coords: HashMap::new(),
        };
        index.replace_base(rows);
        index
    }

    fn replace_base(&mut self, rows: Vec<TileRow>) {
        self.base = CompactBase::build(rows.iter().map(|row| (row.id, row.tile)).collect());
    }

    fn get(&self, id: u64) -> Option<OverworldTile> {
        if let Some(&edit) = self.edits.get(&id) {
            return edit;
        }
        self.base.get_by_id(id)
    }

    fn dirty_at(&mut self, coords: TileCoordinates) -> &mut Vec<(u64, u64)> {
        self.dirty_coords.entry(coords).or_insert_with(|| {
            let mut list: Vec<_> = self.base.base_at(coords).collect();
            if let Some(ids) = self.base.wide_by_coord.get(&coords) {
                list.extend(ids.iter().map(|&id| (id, self.base.wide_by_id[&id].claim)));
            }
            list
        })
    }

    fn replace(&mut self, id: u64, previous: Option<OverworldTile>, next: Option<OverworldTile>) {
        if previous == next {
            return;
        }
        if let Some(old) = previous {
            if let Some(tile) = next.filter(|tile| tile.coords == old.coords) {
                // Same-coordinate update: refresh the occupant entry in place.
                let occupants = self.dirty_at(old.coords);
                let occupant = occupants
                    .iter_mut()
                    .find(|(owner, _)| *owner == id)
                    .expect("existing tile must occupy its coordinates");
                occupant.1 = tile.claim;
                if pack(old.coords).is_none() {
                    // Wide tiles live outside the base; keep that view current.
                    self.base.wide_by_id.insert(id, tile);
                }
                self.edits.insert(id, next);
                self.maybe_rebuild();
                return;
            }
            self.remove(id, old);
        }
        if let Some(tile) = next {
            self.add(id, tile);
        }
        self.edits.insert(id, next);
        self.maybe_rebuild();
    }

    fn remove(&mut self, id: u64, old: OverworldTile) {
        if pack(old.coords).is_none() {
            self.base.wide_by_id.remove(&id);
            if let Some(ids) = self.base.wide_by_coord.get_mut(&old.coords) {
                ids.retain(|&tile_id| tile_id != id);
                if ids.is_empty() {
                    self.base.wide_by_coord.remove(&old.coords);
                }
            }
            // A dirty occupant set must stop reporting the removed claim.
            if let Some(list) = self.dirty_coords.get_mut(&old.coords) {
                list.retain(|&(owner, _)| owner != id);
            }
        } else {
            let occupants = self.dirty_at(old.coords);
            occupants.retain(|&(owner, _)| owner != id);
        }
    }

    fn add(&mut self, id: u64, tile: OverworldTile) {
        if pack(tile.coords).is_none() {
            let claim = tile.claim;
            self.base.wide_by_id.insert(id, tile);
            self.base
                .wide_by_coord
                .entry(tile.coords)
                .or_default()
                .push(id);
            // Keep a dirty occupant set at this coordinate reporting the claim.
            if let Some(list) = self.dirty_coords.get_mut(&tile.coords) {
                list.push((id, claim));
            }
        } else {
            self.dirty_at(tile.coords).push((id, tile.claim));
        }
    }

    fn lookup(&self, coords: TileCoordinates) -> ClaimAtLocation {
        if let Some(rows) = self.dirty_coords.get(&coords) {
            return Self::resolve(rows.iter().map(|&(_, claim)| claim));
        }
        let claims = self.base.base_at(coords).map(|(_, claim)| claim);
        match self.base.wide_by_coord.get(&coords) {
            Some(ids) => {
                Self::resolve(claims.chain(ids.iter().map(|&id| self.base.wide_by_id[&id].claim)))
            }
            None => Self::resolve(claims),
        }
    }

    fn resolve(mut claims: impl Iterator<Item = u64>) -> ClaimAtLocation {
        let Some(first) = claims.next() else {
            return ClaimAtLocation::Unclaimed;
        };
        if claims.all(|claim| claim == first) {
            ClaimAtLocation::Claim(first)
        } else {
            ClaimAtLocation::Unknown
        }
    }

    fn maybe_rebuild(&mut self) {
        if self.edits.len() >= MAX_TILE_EDITS || self.dirty_coords.len() >= MAX_DIRTY_COORDS {
            self.rebuild();
        }
    }

    fn rebuild(&mut self) {
        // Current tiles by ID: the compact base yields ID order and the wide
        // fallback is sorted by ID; apply the overlay in ID order.
        let mut edits: Vec<(u64, Option<OverworldTile>)> =
            std::mem::take(&mut self.edits).into_iter().collect();
        edits.sort_unstable_by_key(|&(id, _)| id);
        let mut wide: Vec<(u64, OverworldTile)> = self
            .base
            .wide_by_id
            .iter()
            .map(|(&id, &tile)| (id, tile))
            .collect();
        wide.sort_unstable_by_key(|&(id, _)| id);
        let mut out =
            Vec::with_capacity(self.base.coords.len() + self.base.wide_by_id.len() + edits.len());
        let mut base_iter = self.base.rows_by_id().peekable();
        let mut wide_iter = wide.into_iter().peekable();
        for (id, edit) in edits {
            while base_iter.peek().is_some_and(|&(base_id, _)| base_id < id) {
                out.push(base_iter.next().unwrap());
            }
            while wide_iter.peek().is_some_and(|&(wide_id, _)| wide_id < id) {
                out.push(wide_iter.next().unwrap());
            }
            // The overlay replaces or removes the base/wide row for this ID.
            if base_iter.peek().is_some_and(|&(base_id, _)| base_id == id) {
                base_iter.next();
            }
            if wide_iter.peek().is_some_and(|&(wide_id, _)| wide_id == id) {
                wide_iter.next();
            }
            if let Some(tile) = edit {
                out.push((id, tile));
            }
        }
        out.extend(base_iter);
        out.extend(wide_iter);
        self.replace_base(
            out.into_iter()
                .map(|(id, tile)| TileRow { id, tile })
                .collect(),
        );
        self.dirty_coords = HashMap::new();
    }

    /// Tiles held by the compact base plus the wide fallback.
    fn rows(&self) -> usize {
        self.base.coords.len() + self.base.wide_by_id.len()
    }

    /// Approximate retained bytes: the four sorted arrays plus the claim
    /// dictionary and the wide fallback maps (overlay capacity excluded).
    fn bytes(&self) -> usize {
        let wide = self.base.wide_by_id.capacity()
            * (std::mem::size_of::<u64>() + std::mem::size_of::<OverworldTile>())
            + self.base.wide_by_coord.capacity()
                * (std::mem::size_of::<TileCoordinates>() + std::mem::size_of::<Vec<u64>>())
            + self
                .base
                .wide_by_coord
                .values()
                .map(|ids| ids.capacity() * std::mem::size_of::<u64>())
                .sum::<usize>();
        std::mem::size_of_val(&*self.base.search_hints)
            + self.base.coords.len() * std::mem::size_of::<u32>()
            + self.base.ids.len() * std::mem::size_of::<u64>()
            + self.base.claim_slots.len() * std::mem::size_of::<u32>()
            + self.base.perm.len() * std::mem::size_of::<u32>()
            + self.base.claims.len() * std::mem::size_of::<u64>()
            + wide
    }
}

// The compact-base index is intentionally the largest variant of the
// snapshot-join state; boxing it would add indirection to the hot lookup path.
#[allow(clippy::large_enum_variant)]
enum Overworld {
    // Join both snapshot streams here. Each known claim has a single entry,
    // whether its location is missing, interior or overworld.
    Loading(HashMap<u64, (u64, Option<TileLocation>)>),
    Ready(SortedOverworld),
}

impl Default for Overworld {
    fn default() -> Self {
        Self::Loading(HashMap::new())
    }
}

impl Overworld {
    fn reserve(&mut self, additional: usize) {
        if let Self::Loading(rows) = self {
            rows.reserve(additional);
        }
    }

    fn lookup(&self, coords: TileCoordinates) -> ClaimAtLocation {
        match self {
            Self::Loading(_) => ClaimAtLocation::Unknown,
            Self::Ready(index) => index.lookup(coords),
        }
    }
}

#[derive(Default)]
struct LiveRegion {
    overworld: Overworld,
    // Interior lookups use descriptions/networks, not tile coordinates. Keep
    // the tile's claim here so a subsequent move can still be indexed.
    interior_tiles: HashMap<u64, u64>,
    missing_locations: HashMap<u64, u64>,
    unmatched_locations: HashMap<u64, TileLocation>,
    unmatched: VecDeque<u64>,
    descriptions: HashMap<u32, u64>,
    networks: HashMap<u64, u64>,
    initial_applied: bool,
    claims_initial_received: bool,
    locations_initial_received: bool,
    descriptions_initial_received: bool,
    networks_initial_received: bool,
}

impl LiveRegion {
    fn store_location(
        &mut self,
        id: u64,
        claim: u64,
        previous: Option<OverworldTile>,
        location: Option<TileLocation>,
    ) {
        let next = match location {
            Some(TileLocation::Overworld(coords)) => Some(OverworldTile { claim, coords }),
            Some(TileLocation::Interior) => {
                self.interior_tiles.insert(id, claim);
                None
            }
            None => {
                self.missing_locations.insert(id, claim);
                None
            }
        };
        let Overworld::Ready(index) = &mut self.overworld else {
            unreachable!("loading tiles keep their location in the pending join")
        };
        index.replace(id, previous, next);
    }

    fn set_claim(&mut self, id: u64, claim: Option<u64>) {
        let index = match &mut self.overworld {
            Overworld::Loading(rows) => {
                match claim {
                    Some(claim) => {
                        rows.entry(id)
                            .and_modify(|entry| entry.0 = claim)
                            .or_insert_with(|| (claim, self.unmatched_locations.remove(&id)));
                    }
                    None => {
                        rows.remove(&id);
                        self.unmatched_locations.remove(&id);
                    }
                }
                return;
            }
            Overworld::Ready(index) => index,
        };
        let previous = index.get(id);
        let Some(claim) = claim else {
            index.replace(id, previous, None);
            self.interior_tiles.remove(&id);
            self.missing_locations.remove(&id);
            self.unmatched_locations.remove(&id);
            return;
        };
        if let Some(tile) = previous {
            index.replace(id, previous, Some(OverworldTile { claim, ..tile }));
        } else if let Some(tile_claim) = self.interior_tiles.get_mut(&id) {
            *tile_claim = claim;
        } else if let Some(location) = self.unmatched_locations.remove(&id) {
            self.store_location(id, claim, None, Some(location));
        } else {
            self.missing_locations.insert(id, claim);
        }
    }

    fn set_location(&mut self, id: u64, location: Option<TileLocation>) {
        let (previous, claim) = match &mut self.overworld {
            Overworld::Loading(rows) => {
                if let Some(entry) = rows.get_mut(&id) {
                    entry.1 = location;
                    return;
                }
                (None, None)
            }
            Overworld::Ready(index) => {
                let previous = index.get(id);
                let claim = previous
                    .map(|tile| tile.claim)
                    .or_else(|| self.interior_tiles.remove(&id))
                    .or_else(|| self.missing_locations.remove(&id));
                (previous, claim)
            }
        };
        match (claim, location) {
            (Some(claim), location) => self.store_location(id, claim, previous, location),
            (None, Some(location)) => {
                self.unmatched_locations.insert(id, location);
                if !self.claims_initial_received {
                    return;
                }
                self.unmatched.push_back(id);
                while self.unmatched.len() > MAX_UNMATCHED_LOCATIONS {
                    if let Some(old) = self.unmatched.pop_front() {
                        self.unmatched_locations.remove(&old);
                    }
                }
            }
            (None, None) => {
                self.unmatched_locations.remove(&id);
            }
        }
    }

    fn finish_claims_initial(&mut self) {
        self.claims_initial_received = true;
        // Matching locations moved into the tile indexes. Release the
        // staging allocation for any remaining building/portal locations.
        self.unmatched_locations = HashMap::new();
        self.unmatched.clear();
    }

    fn finish_overworld_initial(&mut self) {
        if self.claims_initial_received
            && self.locations_initial_received
            && let Overworld::Loading(pending) = &mut self.overworld
        {
            let mut rows = Vec::with_capacity(pending.len());
            for (id, (claim, location)) in std::mem::take(pending) {
                match location {
                    Some(TileLocation::Overworld(coords)) => rows.push(TileRow {
                        id,
                        tile: OverworldTile { claim, coords },
                    }),
                    Some(TileLocation::Interior) => {
                        self.interior_tiles.insert(id, claim);
                    }
                    None => {
                        self.missing_locations.insert(id, claim);
                    }
                }
            }
            self.overworld = Overworld::Ready(SortedOverworld::new(rows));
        }
    }
}

#[derive(Default)]
pub(crate) struct ClaimLocationSubscriptions {
    regions: RwLock<HashMap<Region, RwLock<LiveRegion>>>,
}

impl ClaimLocationSubscriptions {
    pub(crate) fn reset(&self, region: Region) {
        self.regions.write().insert(region, RwLock::default());
    }

    pub(crate) fn invalidate(&self, region: Region) {
        self.regions.write().remove(&region);
    }

    fn update(&self, region: Region, update: impl FnOnce(&mut LiveRegion)) {
        // Hold the registry read lock so a reset cannot replace this region
        // mid-update. Only this region's index is write-locked during a rebuild.
        if let Some(state) = self.regions.read().get(&region) {
            update(&mut state.write());
        }
    }

    pub(crate) fn finish_initial(&self, region: Region) {
        self.update(region, |state| state.initial_applied = true);
    }

    pub(crate) fn claims_initial<'a>(
        &self,
        region: Region,
        rows: impl IntoIterator<Item = &'a game_module::module_bindings::ClaimTileState>,
    ) {
        self.update(region, |state| {
            let rows = rows.into_iter();
            state.overworld.reserve(rows.size_hint().0);
            for row in rows {
                state.set_claim(row.entity_id, Some(row.claim_id));
            }
            state.finish_claims_initial();
            state.finish_overworld_initial();
        });
    }

    pub(crate) fn claim(&self, region: Region, id: u64, claim: Option<u64>) {
        self.update(region, |state| state.set_claim(id, claim));
    }

    pub(crate) fn locations_initial<'a>(
        &self,
        region: Region,
        rows: impl IntoIterator<Item = &'a game_module::module_bindings::LocationState>,
    ) {
        self.update(region, |state| {
            let rows = rows.into_iter();
            let count = rows.size_hint().0;
            if !state.claims_initial_received {
                state.unmatched_locations.reserve(count);
            }
            for row in rows {
                state.set_location(row.entity_id, Some((row.dimension, row.x, row.z).into()));
            }
            state.locations_initial_received = true;
            state.finish_overworld_initial();
        });
    }

    pub(crate) fn location(&self, region: Region, id: u64, location: Option<(u32, i32, i32)>) {
        self.update(region, |state| {
            state.set_location(id, location.map(Into::into));
        });
    }

    pub(crate) fn descriptions_initial<'a>(
        &self,
        region: Region,
        rows: impl IntoIterator<Item = &'a game_module::module_bindings::DimensionDescriptionState>,
    ) {
        self.update(region, |state| {
            for row in rows {
                state
                    .descriptions
                    .insert(row.dimension_id, row.dimension_network_entity_id);
            }
            state.descriptions_initial_received = true;
        });
    }

    pub(crate) fn description(&self, region: Region, dimension: u32, network: Option<u64>) {
        self.update(region, |state| {
            if let Some(network) = network {
                state.descriptions.insert(dimension, network);
            } else {
                state.descriptions.remove(&dimension);
            }
        });
    }

    pub(crate) fn networks_initial<'a>(
        &self,
        region: Region,
        rows: impl IntoIterator<Item = &'a game_module::module_bindings::DimensionNetworkState>,
    ) {
        self.update(region, |state| {
            for row in rows {
                state.networks.insert(row.entity_id, row.claim_entity_id);
            }
            state.networks_initial_received = true;
        });
    }

    pub(crate) fn network(&self, region: Region, id: u64, claim: Option<u64>) {
        self.update(region, |state| {
            if let Some(claim) = claim {
                state.networks.insert(id, claim);
            } else {
                state.networks.remove(&id);
            }
        });
    }

    pub(crate) fn lookup(&self, region: Region, dimension: u32, x: i32, z: i32) -> ClaimAtLocation {
        let regions = self.regions.read();
        let Some(state) = regions.get(&region) else {
            return ClaimAtLocation::Unknown;
        };
        let state = state.read();
        if !state.initial_applied {
            return ClaimAtLocation::Unknown;
        }
        if dimension != 1 {
            if !state.descriptions_initial_received || !state.networks_initial_received {
                return ClaimAtLocation::Unknown;
            }
            return match state
                .descriptions
                .get(&dimension)
                .and_then(|id| state.networks.get(id))
            {
                Some(0) => ClaimAtLocation::Unclaimed,
                Some(claim) => ClaimAtLocation::Claim(*claim),
                None => ClaimAtLocation::Unknown,
            };
        }
        if !state.claims_initial_received
            || !state.locations_initial_received
            || !state.missing_locations.is_empty()
        {
            return ClaimAtLocation::Unknown;
        }
        state.overworld.lookup(mobile_tile(x, z))
    }
}

/// Convert the stored mobile offset coordinates into the small offset tile
/// used by LocationState. This follows BitCraft's FloatHexTile -> world ->
/// HexCoordinates rounding -> OffsetCoordinatesSmall conversion.
pub(crate) fn mobile_tile(location_x: i32, location_z: i32) -> (i32, i32) {
    const OUTER: f32 = 10.0 / 3.0;
    const INNER: f32 = OUTER * 0.866_025_4_f32;
    let axial_x = (location_x - location_z / 2) as f32 / 1000.0;
    let axial_z = location_z as f32 / 1000.0;
    let world_x = 2.0 * axial_x * INNER + axial_z * INNER;
    let world_z = 1.5 * axial_z * OUTER;
    let mut x = world_x / (INNER * 2.0);
    let mut y = -x;
    let offset = world_z / (OUTER * 3.0);
    x -= offset;
    y -= offset;
    let mut ix = x.round() as i32;
    let iy = y.round() as i32;
    let mut iz = (-x - y).round() as i32;
    if ix + iy + iz != 0 {
        let dx = (x - ix as f32).abs();
        let dy = (y - iy as f32).abs();
        let dz = (-x - y - iz as f32).abs();
        if dx > dy && dx > dz {
            ix = -iy - iz;
        } else if dz > dy {
            iz = -ix - iy;
        }
    }
    (ix + iz / 2, iz)
}

/// A complete region snapshot is joined by tile entity ID, then indexed by
/// exact tile coordinates. Duplicate coordinates are ambiguous, not unclaimed.
#[derive(Default)]
pub(crate) struct ClaimLocationSnapshot {
    tiles: HashMap<(Region, u32, i32, i32), Option<u64>>,
    dimensions: HashMap<(Region, u32), u64>,
    region_ready: HashMap<Region, bool>,
}

impl ClaimLocationSnapshot {
    pub(crate) fn replace_region(
        &mut self,
        region: Region,
        claims: impl IntoIterator<Item = (u64, u64)>,
        locations: impl IntoIterator<Item = (u64, u32, i32, i32)>,
        dimensions: impl IntoIterator<Item = (u32, u64)>,
    ) -> bool {
        let locations: HashMap<_, _> = locations
            .into_iter()
            .map(|(id, dimension, x, z)| (id, (dimension, x, z)))
            .collect();
        let mut tiles = HashMap::new();
        let mut complete = true;
        for (id, claim_id) in claims {
            let Some(&(dimension, x, z)) = locations.get(&id) else {
                complete = false;
                continue;
            };
            tiles
                .entry((region, dimension, x, z))
                .and_modify(|claim: &mut Option<u64>| {
                    if *claim != Some(claim_id) {
                        *claim = None;
                    }
                })
                .or_insert(Some(claim_id));
        }
        self.tiles
            .retain(|(key_region, ..), _| *key_region != region);
        self.tiles.extend(tiles);
        self.dimensions
            .retain(|(key_region, _), _| *key_region != region);
        self.dimensions.extend(
            dimensions
                .into_iter()
                .map(|(dimension, claim)| ((region, dimension), claim)),
        );
        self.region_ready.insert(region, complete);
        complete
    }

    pub(crate) fn invalidate_region(&mut self, region: Region) {
        self.region_ready.insert(region, false);
    }

    pub(crate) fn lookup(
        &self,
        region: Region,
        dimension: u32,
        location_x: i32,
        location_z: i32,
    ) -> ClaimAtLocation {
        if !self.region_ready.get(&region).copied().unwrap_or(false) {
            return ClaimAtLocation::Unknown;
        }
        if dimension != 1 {
            return match self.dimensions.get(&(region, dimension)) {
                Some(0) => ClaimAtLocation::Unclaimed,
                Some(claim) => ClaimAtLocation::Claim(*claim),
                None => ClaimAtLocation::Unknown,
            };
        }
        let (x, z) = mobile_tile(location_x, location_z);
        match self.tiles.get(&(region, dimension, x, z)) {
            Some(Some(claim)) => ClaimAtLocation::Claim(*claim),
            Some(None) => ClaimAtLocation::Unknown,
            None => ClaimAtLocation::Unclaimed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_index_waits_for_initial_rows_and_tracks_out_of_order_updates() {
        use game_module::module_bindings::{ClaimTileState, LocationState};
        let index = ClaimLocationSubscriptions::default();
        index.reset(1);
        index.finish_initial(1); // SDK invokes on_applied before initial row callbacks.
        assert_eq!(index.lookup(1, 1, 1000, 2000), ClaimAtLocation::Unknown);
        let locations = [LocationState {
            entity_id: 42,
            chunk_index: 1,
            x: 1,
            z: 2,
            dimension: 1,
        }];
        index.locations_initial(1, &locations);
        assert_eq!(index.lookup(1, 1, 1000, 2000), ClaimAtLocation::Unknown);
        index.claims_initial(
            1,
            &[ClaimTileState {
                entity_id: 42,
                claim_id: 99,
            }],
        );
        assert_eq!(index.lookup(1, 1, 1000, 2000), ClaimAtLocation::Claim(99));
        index.claim(1, 43, Some(100));
        assert_eq!(index.lookup(1, 1, 1000, 2000), ClaimAtLocation::Unknown);
        index.location(1, 43, Some((1, 2, 2)));
        assert_eq!(index.lookup(1, 1, 1000, 2000), ClaimAtLocation::Claim(99));
        index.claim(1, 42, None);
        assert_eq!(index.lookup(1, 1, 1000, 2000), ClaimAtLocation::Unclaimed);
        index.invalidate(1);
        assert_eq!(index.lookup(1, 1, 1000, 2000), ClaimAtLocation::Unknown);
    }

    #[test]
    fn releases_unrelated_initial_locations_after_the_join() {
        use game_module::module_bindings::{ClaimTileState, LocationState};

        let index = ClaimLocationSubscriptions::default();
        index.reset(1);
        index.finish_initial(1);
        let locations: Vec<_> = (0..128)
            .map(|id| LocationState {
                entity_id: id,
                chunk_index: 1,
                x: id as i32,
                z: 0,
                dimension: 1,
            })
            .collect();
        // Building and portal locations may arrive alongside claim tiles.
        index.locations_initial(1, &locations);
        index.claims_initial(
            1,
            &[ClaimTileState {
                entity_id: 1,
                claim_id: 99,
            }],
        );
        assert_eq!(index.lookup(1, 1, 1000, 0), ClaimAtLocation::Claim(99));
        assert_eq!(index.lookup(1, 1, 2000, 0), ClaimAtLocation::Unclaimed);
        let regions = index.regions.read();
        let region = regions[&1].read();
        let Overworld::Ready(overworld) = &region.overworld else {
            panic!("the initial join should build the sorted index");
        };
        assert_eq!(overworld.rows(), 1);
        assert_eq!(unpack(overworld.base.coords[0]), (1, 0));
        assert_eq!(region.unmatched_locations.capacity(), 0);
        assert_eq!(region.missing_locations.capacity(), 0);
    }

    #[test]
    fn updates_between_initial_callbacks_survive_publication() {
        use game_module::module_bindings::{ClaimTileState, LocationState};

        for claims_first in [true, false] {
            let index = ClaimLocationSubscriptions::default();
            index.reset(1);
            index.finish_initial(1);
            let claims = [ClaimTileState {
                entity_id: 2,
                claim_id: 8,
            }];
            let locations = [LocationState {
                entity_id: 2,
                chunk_index: 0,
                x: -2,
                z: 2,
                dimension: 1,
            }];
            if claims_first {
                index.claims_initial(1, &claims);
                index.claim(1, 2, Some(9));
                index.location(1, 3, Some((1, 1, -2)));
                index.claim(1, 3, Some(17));
                index.locations_initial(1, &locations);
            } else {
                index.locations_initial(1, &locations);
                index.location(1, 2, Some((1, -3, 2)));
                index.claim(1, 3, Some(17));
                index.location(1, 3, Some((1, 1, -2)));
                index.claims_initial(1, &claims);
            }
            assert_eq!(index.lookup(1, 1, 1000, -2000), ClaimAtLocation::Claim(17));
            let (x, claim) = if claims_first { (-2, 9) } else { (-3, 8) };
            assert_eq!(
                index.lookup(1, 1, x * 1000, 2000),
                ClaimAtLocation::Claim(claim)
            );
        }
    }

    #[test]
    fn tiles_can_move_between_the_overworld_and_interiors() {
        let index = ClaimLocationSubscriptions::default();
        index.reset(1);
        index.claims_initial(1, []);
        index.locations_initial(1, []);
        index.finish_initial(1);
        index.claim(1, 42, Some(99));
        assert_eq!(index.lookup(1, 1, 1000, 2000), ClaimAtLocation::Unknown);
        index.location(1, 42, Some((7, 1, 2)));
        assert_eq!(index.lookup(1, 1, 1000, 2000), ClaimAtLocation::Unclaimed);
        index.claim(1, 42, Some(101));
        index.location(1, 42, Some((1, 1, 2)));
        assert_eq!(index.lookup(1, 1, 1000, 2000), ClaimAtLocation::Claim(101));
        index.location(1, 42, Some((7, 1, 2)));
        assert_eq!(index.lookup(1, 1, 1000, 2000), ClaimAtLocation::Unclaimed);
        index.location(1, 42, None);
        assert_eq!(index.lookup(1, 1, 1000, 2000), ClaimAtLocation::Unknown);
        index.claim(1, 42, None);
        assert_eq!(index.lookup(1, 1, 1000, 2000), ClaimAtLocation::Unclaimed);
    }

    #[test]
    fn overlapping_tiles_preserve_ambiguity_through_moves_and_deletes() {
        use game_module::module_bindings::{ClaimTileState, LocationState};
        let index = ClaimLocationSubscriptions::default();
        index.reset(1);
        index.claims_initial(
            1,
            &(1..=3)
                .map(|entity_id| ClaimTileState {
                    entity_id,
                    claim_id: 99,
                })
                .collect::<Vec<_>>(),
        );
        index.locations_initial(
            1,
            &(1..=3)
                .map(|entity_id| LocationState {
                    entity_id,
                    chunk_index: 1,
                    x: 1,
                    z: 2,
                    dimension: 1,
                })
                .collect::<Vec<_>>(),
        );
        index.finish_initial(1);
        assert_eq!(index.lookup(1, 1, 1000, 2000), ClaimAtLocation::Claim(99));
        index.claim(1, 2, Some(100));
        assert_eq!(index.lookup(1, 1, 1000, 2000), ClaimAtLocation::Unknown);
        index.location(1, 2, Some((1, 5, 2)));
        assert_eq!(index.lookup(1, 1, 1000, 2000), ClaimAtLocation::Claim(99));
        assert_eq!(index.lookup(1, 1, 5000, 2000), ClaimAtLocation::Claim(100));
        index.claim(1, 1, None); // promote another tile at the old coordinate
        assert_eq!(index.lookup(1, 1, 1000, 2000), ClaimAtLocation::Claim(99));
        index.claim(1, 3, Some(101));
        assert_eq!(index.lookup(1, 1, 1000, 2000), ClaimAtLocation::Claim(101));
        index.claim(1, 3, None);
        assert_eq!(index.lookup(1, 1, 1000, 2000), ClaimAtLocation::Unclaimed);
        assert_eq!(index.lookup(1, 1, 5000, 2000), ClaimAtLocation::Claim(100));
    }

    #[test]
    fn live_mutations_match_rebuilt_snapshot_with_duplicate_coordinates() {
        let index = ClaimLocationSubscriptions::default();
        index.reset(1);
        index.claims_initial(1, []);
        index.locations_initial(1, []);
        index.finish_initial(1);
        let mut claims = HashMap::new();
        let mut locations = HashMap::new();
        let mut random = 42u64;
        for step in 0..2000 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            let id = (random >> 32) % 16;
            let x = ((random >> 40) % 4) as i32;
            let claim = (random >> 48) % 3;
            match (random >> 56) % 4 {
                0 => {
                    claims.insert(id, claim);
                    index.claim(1, id, Some(claim));
                }
                1 => {
                    claims.remove(&id);
                    locations.remove(&id);
                    index.claim(1, id, None);
                }
                2 => {
                    locations.insert(id, (1, x, 0));
                    index.location(1, id, Some((1, x, 0)));
                }
                _ => {
                    locations.remove(&id);
                    index.location(1, id, None);
                }
            }
            // Regularly complete outstanding joins to exercise known results,
            // not just the conservative Unknown result for incomplete regions.
            if step % 7 == 0 {
                for &id in claims.keys() {
                    let coords = *locations.entry(id).or_insert((1, (id % 4) as i32, 0));
                    index.location(1, id, Some(coords));
                }
            }
            if step % 57 == 0 {
                index.update(1, |state| {
                    if let Overworld::Ready(overworld) = &mut state.overworld {
                        overworld.rebuild();
                    }
                });
            }
            let mut snapshot = ClaimLocationSnapshot::default();
            snapshot.replace_region(
                1,
                claims.iter().map(|(&id, &claim)| (id, claim)),
                locations.iter().map(|(&id, &(dim, x, z))| (id, dim, x, z)),
                [],
            );
            for x in 0..5 {
                assert_eq!(
                    index.lookup(1, 1, x * 1000, 0),
                    snapshot.lookup(1, 1, x * 1000, 0),
                    "step {step}, tile {x}"
                );
            }
        }
    }

    #[test]
    fn large_base_hints_follow_edits_overlaps_and_rebuilds() {
        let mut index = SortedOverworld::new(
            (0..5000)
                .map(|id| TileRow {
                    id,
                    tile: OverworldTile {
                        claim: id,
                        coords: (id as i32, 0),
                    },
                })
                .collect(),
        );
        for x in (0..5000).chain((0..5000).rev()) {
            assert_eq!(index.lookup((x, 0)), ClaimAtLocation::Claim(x as u64));
        }
        // Warm a miss and a hit before moving their occupants via the overlay.
        let target = (-1, -1);
        assert_eq!(index.lookup(target), ClaimAtLocation::Unclaimed);
        for id in [0, 1234] {
            let old = index.get(id).unwrap();
            assert_eq!(index.lookup(old.coords), ClaimAtLocation::Claim(id));
            index.replace(
                id,
                Some(old),
                Some(OverworldTile {
                    claim: id,
                    coords: target,
                }),
            );
            assert_eq!(index.lookup(old.coords), ClaimAtLocation::Unclaimed);
        }
        assert_eq!(index.lookup(target), ClaimAtLocation::Unknown);
        index.rebuild();
        assert_eq!(index.lookup(target), ClaimAtLocation::Unknown);
        assert_eq!(index.lookup((0, 0)), ClaimAtLocation::Unclaimed);
        assert_eq!(index.lookup((1234, 0)), ClaimAtLocation::Unclaimed);
        assert_eq!(index.lookup((1235, 0)), ClaimAtLocation::Claim(1235));
    }

    #[test]
    fn repeated_moves_rebuild_when_dirty_coordinates_reach_the_limit() {
        let start = OverworldTile {
            claim: 17,
            coords: (0, 0),
        };
        let mut index = SortedOverworld::new(vec![TileRow { id: 1, tile: start }]);
        for x in 1..MAX_DIRTY_COORDS as i32 {
            let previous = index.get(1);
            let next = OverworldTile {
                coords: (x, 0),
                ..start
            };
            index.replace(1, previous, Some(next));
            assert_eq!(index.lookup((x - 1, 0)), ClaimAtLocation::Unclaimed);
            assert_eq!(index.lookup((x, 0)), ClaimAtLocation::Claim(17));
        }
        assert!(index.edits.is_empty() && index.dirty_coords.is_empty());
        assert_eq!(index.rows(), 1);
        assert_eq!(
            unpack(index.base.coords[0]),
            (MAX_DIRTY_COORDS as i32 - 1, 0)
        );
    }

    #[test]
    fn distinct_edits_rebuild_and_keep_deleted_coordinates_empty() {
        let rows = (0..MAX_TILE_EDITS as i32)
            .map(|x| TileRow {
                id: x as u64,
                tile: OverworldTile {
                    claim: 17,
                    coords: (-x, i32::MIN),
                },
            })
            .collect();
        let mut index = SortedOverworld::new(rows);
        for id in 0..MAX_TILE_EDITS as u64 {
            let previous = index.get(id);
            index.replace(id, previous, None);
        }
        assert!(index.edits.is_empty() && index.dirty_coords.is_empty());
        assert!(index.base.coords.is_empty() && index.base.ids.is_empty());
        assert_eq!(index.lookup((0, i32::MIN)), ClaimAtLocation::Unclaimed);
    }

    #[test]
    fn interior_lookup_joins_dimension_description_to_network() {
        let index = ClaimLocationSubscriptions::default();
        index.reset(2);
        index.finish_initial(2);
        index.description(2, 15, Some(1000));
        index.network(2, 1000, Some(99));
        assert_eq!(index.lookup(2, 15, 0, 0), ClaimAtLocation::Unknown);
        index.update(2, |region| {
            region.descriptions_initial_received = true;
            region.networks_initial_received = true;
        });
        assert_eq!(index.lookup(2, 15, 0, 0), ClaimAtLocation::Claim(99));
        index.network(2, 1000, Some(0));
        assert_eq!(index.lookup(2, 15, 0, 0), ClaimAtLocation::Unclaimed);
        index.network(2, 1000, None);
        assert_eq!(index.lookup(2, 15, 0, 0), ClaimAtLocation::Unknown);
    }

    #[test]
    fn converts_mobile_coordinates_to_tile_centers() {
        assert_eq!(mobile_tile(0, 0), (0, 0));
        assert_eq!(mobile_tile(1000, 2000), (1, 2));
        assert_eq!(mobile_tile(-1000, -2000), (-1, -2));
    }

    #[test]
    fn joins_by_entity_and_distinguishes_unknown_from_unclaimed() {
        let mut snapshot = ClaimLocationSnapshot::default();
        assert_eq!(snapshot.lookup(1, 1, 1000, 2000), ClaimAtLocation::Unknown);
        assert!(!snapshot.replace_region(1, [(42, 99)], [], []));
        assert_eq!(snapshot.lookup(1, 1, 1000, 2000), ClaimAtLocation::Unknown);
        assert!(snapshot.replace_region(1, [(42, 99)], [(42, 1, 1, 2)], [(3, 123), (4, 0)]));
        assert_eq!(
            snapshot.lookup(1, 1, 1000, 2000),
            ClaimAtLocation::Claim(99)
        );
        assert_eq!(
            snapshot.lookup(1, 1, 9000, 9000),
            ClaimAtLocation::Unclaimed
        );
        assert_eq!(snapshot.lookup(2, 1, 1000, 2000), ClaimAtLocation::Unknown);
        assert_eq!(snapshot.lookup(1, 3, 0, 0), ClaimAtLocation::Claim(123));
        assert_eq!(snapshot.lookup(1, 4, 0, 0), ClaimAtLocation::Unclaimed);
        snapshot.invalidate_region(1);
        assert_eq!(snapshot.lookup(1, 1, 1000, 2000), ClaimAtLocation::Unknown);
    }

    #[test]
    fn pack_unpack_preserves_narrow_and_wide_coordinates() {
        for coords in [
            (0, 0),
            (1, 2),
            (-1, -2),
            (32_767, -32_768),
            (-32_768, 32_767),
            (32_767, 32_767),
            (-32_768, -32_768),
            (100, -100),
        ] {
            assert!(pack(coords).is_some());
            assert_eq!(unpack(pack(coords).unwrap()), coords);
        }
        assert_eq!(pack((32_768, 0)), None);
        assert_eq!(pack((-32_769, 0)), None);
        assert_eq!(pack((0, 32_768)), None);
        assert_eq!(pack((0, -32_769)), None);
        assert_eq!(pack((i32::MAX, 0)), None);
        assert_eq!(pack((0, i32::MIN)), None);
    }

    #[test]
    fn wide_coordinates_use_fallback_for_lookups_and_edits() {
        let mut index = SortedOverworld::new(vec![TileRow {
            id: 1,
            tile: OverworldTile {
                claim: 17,
                coords: (100_000, 0),
            },
        }]);
        assert_eq!(index.lookup((100_000, 0)), ClaimAtLocation::Claim(17));
        assert_eq!(index.lookup((100_001, 0)), ClaimAtLocation::Unclaimed);

        // Same-coordinate wide update through the fast path.
        let previous = index.get(1);
        index.replace(
            1,
            previous,
            Some(OverworldTile {
                claim: 18,
                coords: (100_000, 0),
            }),
        );
        assert_eq!(index.lookup((100_000, 0)), ClaimAtLocation::Claim(18));
        assert_eq!(index.get(1).unwrap().claim, 18);

        // Wide to narrow move.
        let previous = index.get(1);
        index.replace(
            1,
            previous,
            Some(OverworldTile {
                claim: 19,
                coords: (3, 0),
            }),
        );
        assert_eq!(index.lookup((3, 0)), ClaimAtLocation::Claim(19));
        assert_eq!(index.lookup((100_000, 0)), ClaimAtLocation::Unclaimed);

        // Narrow to wide move.
        let previous = index.get(1);
        index.replace(
            1,
            previous,
            Some(OverworldTile {
                claim: 20,
                coords: (-50_000, -40_000),
            }),
        );
        assert_eq!(index.lookup((-50_000, -40_000)), ClaimAtLocation::Claim(20));
        assert_eq!(index.lookup((3, 0)), ClaimAtLocation::Unclaimed);

        // Delete the wide tile.
        let previous = index.get(1);
        index.replace(1, previous, None);
        assert_eq!(index.lookup((-50_000, -40_000)), ClaimAtLocation::Unclaimed);
        assert!(index.get(1).is_none());
        assert_eq!(index.rows(), 0);
    }

    #[test]
    fn wide_and_negative_coordinates_survive_rebuild() {
        let mut index = SortedOverworld::new(vec![
            TileRow {
                id: 1,
                tile: OverworldTile {
                    claim: 1,
                    coords: (2, 3),
                },
            },
            TileRow {
                id: 2,
                tile: OverworldTile {
                    claim: 2,
                    coords: (-2, -3),
                },
            },
            TileRow {
                id: 3,
                tile: OverworldTile {
                    claim: 3,
                    coords: (32_767, -32_768),
                },
            },
            TileRow {
                id: 4,
                tile: OverworldTile {
                    claim: 4,
                    coords: (32_768, 0),
                },
            },
            TileRow {
                id: 5,
                tile: OverworldTile {
                    claim: 5,
                    coords: (-32_769, 0),
                },
            },
        ]);
        let expected = [
            ((2, 3), 1),
            ((-2, -3), 2),
            ((32_767, -32_768), 3),
            ((32_768, 0), 4),
            ((-32_769, 0), 5),
        ];
        for &(coords, claim) in &expected {
            assert_eq!(index.lookup(coords), ClaimAtLocation::Claim(claim));
        }
        assert_eq!(index.rows(), 5);
        index.rebuild();
        for &(coords, claim) in &expected {
            assert_eq!(
                index.lookup(coords),
                ClaimAtLocation::Claim(claim),
                "after rebuild at {coords:?}"
            );
        }
        assert_eq!(index.rows(), 5);
        assert_eq!(index.get(4).unwrap().coords, (32_768, 0));
        assert_eq!(index.get(2).unwrap().coords, (-2, -3));
    }

    #[test]
    fn dirty_wide_and_narrow_occupants_mix_to_unknown() {
        // Tile 7 is wide; the same-coordinate wide update builds the dirty
        // occupant list from the wide view first.
        let mut index = SortedOverworld::new(vec![TileRow {
            id: 7,
            tile: OverworldTile {
                claim: 10,
                coords: (40_000, 0),
            },
        }]);
        let previous = index.get(7);
        index.replace(
            7,
            previous,
            Some(OverworldTile {
                claim: 11,
                coords: (40_000, 0),
            }),
        );
        assert_eq!(index.lookup((40_000, 0)), ClaimAtLocation::Claim(11));

        // A narrow tile moving into the same coordinate sees both occupants.
        index.replace(
            8,
            None,
            Some(OverworldTile {
                claim: 12,
                coords: (40_000, 0),
            }),
        );
        assert_eq!(index.lookup((40_000, 0)), ClaimAtLocation::Unknown);
        let previous = index.get(8);
        index.replace(8, previous, None);
        assert_eq!(index.lookup((40_000, 0)), ClaimAtLocation::Claim(11));
    }

    #[test]
    fn live_mutations_match_rebuilt_snapshot_with_wide_and_negative_coordinates() {
        let index = ClaimLocationSubscriptions::default();
        index.reset(1);
        index.claims_initial(1, []);
        index.locations_initial(1, []);
        index.finish_initial(1);
        let mut claims = HashMap::new();
        let mut locations = HashMap::new();
        let mut random = 0x5EED_u64;
        for step in 0..2000 {
            random = random
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let id = (random >> 32) % 16;
            let x = ((random >> 40) % 7) as i32 - 3;
            let claim = (random >> 48) % 3;
            let wide = ((random >> 52) % 8) == 0;
            let wide_coord = |r: u64| (((r % 4) as i32) - 2) * 400_000;
            match (random >> 56) % 4 {
                0 => {
                    claims.insert(id, claim);
                    index.claim(1, id, Some(claim));
                }
                1 => {
                    claims.remove(&id);
                    locations.remove(&id);
                    index.claim(1, id, None);
                }
                2 => {
                    let (x, z) = if wide {
                        (wide_coord(random), 0)
                    } else {
                        (x, 0)
                    };
                    locations.insert(id, (1, x, z));
                    index.location(1, id, Some((1, x, z)));
                }
                _ => {
                    locations.remove(&id);
                    index.location(1, id, None);
                }
            }
            // Regularly complete outstanding joins to exercise known results,
            // not just the conservative Unknown result for incomplete regions.
            if step % 7 == 0 {
                for &id in claims.keys() {
                    let coords = *locations.entry(id).or_insert((1, (id % 7) as i32 - 3, 0));
                    index.location(1, id, Some(coords));
                }
            }
            if step % 57 == 0 {
                index.update(1, |state| {
                    if let Overworld::Ready(overworld) = &mut state.overworld {
                        overworld.rebuild();
                    }
                });
            }
            let mut snapshot = ClaimLocationSnapshot::default();
            snapshot.replace_region(
                1,
                claims.iter().map(|(&id, &claim)| (id, claim)),
                locations.iter().map(|(&id, &(dim, x, z))| (id, dim, x, z)),
                [],
            );
            for (x, z) in [
                (-3, 0),
                (3, 0),
                (7, 0),
                (-400_000, 0),
                (400_000, 0),
                (8_191, 0),
                (8_191, 8_191),
            ] {
                assert_eq!(
                    index.lookup(1, 1, x, z),
                    snapshot.lookup(1, 1, x, z),
                    "step {step}, tile ({x}, {z})"
                );
            }
        }
    }
}

#[cfg(test)]
mod benchmarks;
