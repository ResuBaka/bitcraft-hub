//! Claim lookup primitives. An absent or incomplete snapshot must never be
//! interpreted as proof that a location is unclaimed.
#![allow(dead_code)] // Includes standalone snapshot helpers used by tests.

use entity::shared::Region;
use parking_lot::RwLock;
use std::collections::{HashMap, VecDeque};

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

struct SortedOverworld {
    by_id: Box<[TileRow]>,
    // Store the keys directly so binary search doesn't chase row pointers.
    coords: Box<[u64]>,
    rows_by_coord: Box<[u32]>,
    edits: HashMap<u64, Option<OverworldTile>>,
    // An empty list masks a coordinate whose base occupants were all removed.
    dirty_coords: HashMap<TileCoordinates, Vec<(u64, u64)>>,
}

impl SortedOverworld {
    fn new(mut rows: Vec<TileRow>) -> Self {
        rows.sort_unstable_by_key(|row| row.id);
        let mut index = Self {
            by_id: Box::new([]),
            coords: Box::new([]),
            rows_by_coord: Box::new([]),
            edits: HashMap::new(),
            dirty_coords: HashMap::new(),
        };
        index.replace_base(rows);
        index
    }

    fn replace_base(&mut self, rows: Vec<TileRow>) {
        let count = u32::try_from(rows.len()).expect("a region has too many overworld tiles");
        // Sort the coordinates themselves rather than chasing slots into the
        // ID-sorted rows at each comparison. Keep lookup keys packed after the sort.
        let mut keyed: Vec<_> = rows.iter().zip(0..count)
            .map(|(row, slot)| (coordinate_key(row.tile.coords), slot))
            .collect();
        keyed.sort_unstable_by_key(|&(key, _)| key);
        self.coords = keyed.iter().map(|&(key, _)| key).collect();
        self.rows_by_coord = keyed.into_iter().map(|(_, slot)| slot).collect();
        self.by_id = rows.into_boxed_slice();
    }

    fn get(&self, id: u64) -> Option<OverworldTile> {
        if let Some(&edit) = self.edits.get(&id) {
            edit
        } else {
            let slot = self.by_id.binary_search_by_key(&id, |row| row.id).ok()?;
            Some(self.by_id[slot].tile)
        }
    }

    fn base_at<'a>(
        rows: &'a [TileRow],
        keys: &'a [u64],
        order: &'a [u32],
        coords: TileCoordinates,
    ) -> impl Iterator<Item = TileRow> + 'a {
        let coords = coordinate_key(coords);
        let start = keys.partition_point(|&key| key < coords);
        keys[start..]
            .iter()
            .zip(&order[start..])
            .take_while(move |(key, _)| **key == coords)
            .map(|(_, &slot)| rows[slot as usize])
    }

    fn dirty_at(&mut self, coords: TileCoordinates) -> &mut Vec<(u64, u64)> {
        self.dirty_coords.entry(coords).or_insert_with(|| {
            Self::base_at(&self.by_id, &self.coords, &self.rows_by_coord, coords)
                .map(|row| (row.id, row.tile.claim))
                .collect()
        })
    }

    fn replace(&mut self, id: u64, previous: Option<OverworldTile>, next: Option<OverworldTile>) {
        if previous == next {
            return;
        }
        if let Some(old) = previous {
            let occupants = self.dirty_at(old.coords);
            if let Some(tile) = next.filter(|tile| tile.coords == old.coords) {
                let occupant = occupants.iter_mut().find(|(owner, _)| *owner == id)
                    .expect("existing tile must occupy its coordinates");
                occupant.1 = tile.claim;
                self.edits.insert(id, next);
                self.maybe_rebuild();
                return;
            }
            occupants.retain(|(owner, _)| *owner != id);
        }
        if let Some(tile) = next {
            self.dirty_at(tile.coords).push((id, tile.claim));
        }
        self.edits.insert(id, next);
        self.maybe_rebuild();
    }

    fn lookup(&self, coords: TileCoordinates) -> ClaimAtLocation {
        if let Some(rows) = self.dirty_coords.get(&coords) {
            Self::resolve(rows.iter().map(|&(_, claim)| claim))
        } else {
            Self::resolve(
                Self::base_at(&self.by_id, &self.coords, &self.rows_by_coord, coords)
                    .map(|row| row.tile.claim),
            )
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
        let mut edits: Vec<_> = std::mem::take(&mut self.edits).into_iter().collect();
        edits.sort_unstable_by_key(|&(id, _)| id);
        let mut rows = Vec::with_capacity(self.by_id.len() + edits.len());
        let mut base = self.by_id.iter().copied().peekable();
        for (id, tile) in edits {
            while base.peek().is_some_and(|row| row.id < id) {
                rows.push(base.next().unwrap());
            }
            if base.peek().is_some_and(|row| row.id == id) {
                base.next();
            }
            if let Some(tile) = tile {
                rows.push(TileRow { id, tile });
            }
        }
        rows.extend(base);
        self.replace_base(rows);
        self.dirty_coords = HashMap::new();
    }
}

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
                let claim = previous.map(|tile| tile.claim)
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
        if self.claims_initial_received && self.locations_initial_received
            && let Overworld::Loading(pending) = &mut self.overworld
        {
            let mut rows = Vec::with_capacity(pending.len());
            for (id, (claim, location)) in std::mem::take(pending) {
                match location {
                    Some(TileLocation::Overworld(coords)) => rows.push(TileRow { id, tile: OverworldTile { claim, coords } }),
                    Some(TileLocation::Interior) => { self.interior_tiles.insert(id, claim); }
                    None => { self.missing_locations.insert(id, claim); }
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
                state.descriptions.insert(row.dimension_id, row.dimension_network_entity_id);
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
            return match state.descriptions.get(&dimension).and_then(|id| state.networks.get(id)) {
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
        let locations: HashMap<_, _> = locations.into_iter()
            .map(|(id, dimension, x, z)| (id, (dimension, x, z))).collect();
        let mut tiles = HashMap::new();
        let mut complete = true;
        for (id, claim_id) in claims {
            let Some(&(dimension, x, z)) = locations.get(&id) else {
                complete = false;
                continue;
            };
            tiles.entry((region, dimension, x, z))
                .and_modify(|claim: &mut Option<u64>| {
                    if *claim != Some(claim_id) { *claim = None; }
                })
                .or_insert(Some(claim_id));
        }
        self.tiles.retain(|(key_region, ..), _| *key_region != region);
        self.tiles.extend(tiles);
        self.dimensions.retain(|(key_region, _), _| *key_region != region);
        self.dimensions.extend(dimensions.into_iter().map(|(dimension, claim)| ((region, dimension), claim)));
        self.region_ready.insert(region, complete);
        complete
    }

    pub(crate) fn invalidate_region(&mut self, region: Region) {
        self.region_ready.insert(region, false);
    }

    pub(crate) fn lookup(&self, region: Region, dimension: u32, location_x: i32, location_z: i32) -> ClaimAtLocation {
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
        let locations = [LocationState { entity_id: 42, chunk_index: 1, x: 1, z: 2, dimension: 1 }];
        index.locations_initial(1, &locations);
        assert_eq!(index.lookup(1, 1, 1000, 2000), ClaimAtLocation::Unknown);
        index.claims_initial(1, &[ClaimTileState { entity_id: 42, claim_id: 99 }]);
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
        index.claims_initial(1, &[ClaimTileState { entity_id: 1, claim_id: 99 }]);
        assert_eq!(index.lookup(1, 1, 1000, 0), ClaimAtLocation::Claim(99));
        assert_eq!(index.lookup(1, 1, 2000, 0), ClaimAtLocation::Unclaimed);
        let regions = index.regions.read();
        let region = regions[&1].read();
        let Overworld::Ready(overworld) = &region.overworld else {
            panic!("the initial join should build the sorted index");
        };
        assert_eq!(overworld.by_id.len(), 1);
        assert_eq!(&*overworld.coords, &[coordinate_key((1, 0))]);
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
            let claims = [ClaimTileState { entity_id: 2, claim_id: 8 }];
            let locations = [LocationState { entity_id: 2, chunk_index: 0, x: -2, z: 2, dimension: 1 }];
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
            assert_eq!(index.lookup(1, 1, x * 1000, 2000), ClaimAtLocation::Claim(claim));
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
        index.claims_initial(1, &(1..=3).map(|entity_id| ClaimTileState {
            entity_id, claim_id: 99,
        }).collect::<Vec<_>>());
        index.locations_initial(1, &(1..=3).map(|entity_id| LocationState {
            entity_id, chunk_index: 1, x: 1, z: 2, dimension: 1,
        }).collect::<Vec<_>>());
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
                0 => { claims.insert(id, claim); index.claim(1, id, Some(claim)); }
                1 => {
                    claims.remove(&id);
                    locations.remove(&id);
                    index.claim(1, id, None);
                }
                2 => {
                    locations.insert(id, (1, x, 0));
                    index.location(1, id, Some((1, x, 0)));
                }
                _ => { locations.remove(&id); index.location(1, id, None); }
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
            snapshot.replace_region(1, claims.iter().map(|(&id, &claim)| (id, claim)),
                locations.iter().map(|(&id, &(dim, x, z))| (id, dim, x, z)), []);
            for x in 0..5 {
                assert_eq!(index.lookup(1, 1, x * 1000, 0), snapshot.lookup(1, 1, x * 1000, 0),
                    "step {step}, tile {x}");
            }
        }
    }

    #[test]
    fn repeated_moves_rebuild_when_dirty_coordinates_reach_the_limit() {
        let start = OverworldTile { claim: 17, coords: (0, 0) };
        let mut index = SortedOverworld::new(vec![TileRow { id: 1, tile: start }]);
        for x in 1..MAX_DIRTY_COORDS as i32 {
            let previous = index.get(1);
            let next = OverworldTile { coords: (x, 0), ..start };
            index.replace(1, previous, Some(next));
            assert_eq!(index.lookup((x - 1, 0)), ClaimAtLocation::Unclaimed);
            assert_eq!(index.lookup((x, 0)), ClaimAtLocation::Claim(17));
        }
        assert!(index.edits.is_empty() && index.dirty_coords.is_empty());
        assert_eq!(&*index.coords, &[coordinate_key((MAX_DIRTY_COORDS as i32 - 1, 0))]);
    }

    #[test]
    fn distinct_edits_rebuild_and_keep_deleted_coordinates_empty() {
        let rows = (0..MAX_TILE_EDITS as i32).map(|x| TileRow {
            id: x as u64,
            tile: OverworldTile { claim: 17, coords: (-x, i32::MIN) },
        }).collect();
        let mut index = SortedOverworld::new(rows);
        for id in 0..MAX_TILE_EDITS as u64 {
            let previous = index.get(id);
            index.replace(id, previous, None);
        }
        assert!(index.edits.is_empty() && index.dirty_coords.is_empty());
        assert!(index.by_id.is_empty() && index.coords.is_empty());
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
        assert_eq!(snapshot.lookup(1, 1, 1000, 2000), ClaimAtLocation::Claim(99));
        assert_eq!(snapshot.lookup(1, 1, 9000, 9000), ClaimAtLocation::Unclaimed);
        assert_eq!(snapshot.lookup(2, 1, 1000, 2000), ClaimAtLocation::Unknown);
        assert_eq!(snapshot.lookup(1, 3, 0, 0), ClaimAtLocation::Claim(123));
        assert_eq!(snapshot.lookup(1, 4, 0, 0), ClaimAtLocation::Unclaimed);
        snapshot.invalidate_region(1);
        assert_eq!(snapshot.lookup(1, 1, 1000, 2000), ClaimAtLocation::Unknown);
    }
}
