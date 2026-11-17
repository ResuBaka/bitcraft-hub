//! Claim lookup primitives. An absent or incomplete snapshot must never be
//! interpreted as proof that a location is unclaimed.
#![allow(dead_code)] // Includes standalone snapshot helpers used by tests.

use entity::shared::Region;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClaimAtLocation {
    Claim(u64),
    Unclaimed,
    Unknown,
}

type TileCoordinates = (u32, i32, i32);
const MAX_UNMATCHED_LOCATIONS: usize = 4096;

#[derive(Default)]
struct LiveRegion {
    claims: HashMap<u64, u64>,
    locations: HashMap<u64, TileCoordinates>,
    // Almost every coordinate has one tile. Store its ID inline instead of
    // allocating a separate hash table (and duplicating its claim) per tile.
    coordinates: HashMap<TileCoordinates, u64>,
    overlapping: HashMap<TileCoordinates, HashSet<u64>>,
    missing: HashSet<u64>,
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
    fn detach(&mut self, id: u64) {
        let Some(coords) = self.locations.get(&id) else { return; };
        let Some(&representative) = self.coordinates.get(coords) else { return; };
        if representative == id {
            let replacement = self.overlapping.get_mut(coords).and_then(|others| {
                let next = others.iter().next().copied()?;
                others.remove(&next);
                Some(next)
            });
            if let Some(next) = replacement {
                self.coordinates.insert(*coords, next);
            } else {
                self.coordinates.remove(coords);
            }
        } else if let Some(others) = self.overlapping.get_mut(coords) {
            others.remove(&id);
        }
        if self.overlapping.get(coords).is_some_and(HashSet::is_empty) {
            self.overlapping.remove(coords);
        }
    }

    fn attach(&mut self, id: u64) {
        match (self.claims.get(&id), self.locations.get(&id)) {
            (Some(_), Some(&coords)) => {
                let representative = self.coordinates.entry(coords).or_insert(id);
                if *representative != id {
                    self.overlapping.entry(coords).or_default().insert(id);
                }
                self.missing.remove(&id);
            }
            (Some(_), None) => { self.missing.insert(id); }
            _ => { self.missing.remove(&id); }
        }
    }

    fn set_claim(&mut self, id: u64, claim: Option<u64>) {
        self.detach(id);
        if let Some(claim) = claim { self.claims.insert(id, claim); }
        else { self.claims.remove(&id); self.locations.remove(&id); }
        self.attach(id);
    }

    fn set_location(&mut self, id: u64, location: Option<TileCoordinates>) {
        self.detach(id);
        if let Some(location) = location {
            self.locations.insert(id, location);
            if self.claims_initial_received && !self.claims.contains_key(&id) {
                self.unmatched.push_back(id);
                while self.unmatched.len() > MAX_UNMATCHED_LOCATIONS {
                    if let Some(old) = self.unmatched.pop_front() && !self.claims.contains_key(&old) {
                        self.locations.remove(&old);
                    }
                }
            }
        } else { self.locations.remove(&id); }
        self.attach(id);
    }

    fn finish_claims_initial(&mut self) {
        self.claims_initial_received = true;
        self.locations.retain(|id, _| self.claims.contains_key(id));
        self.unmatched.clear();
    }
}

#[derive(Default)]
pub(crate) struct ClaimLocationSubscriptions {
    regions: parking_lot::RwLock<HashMap<Region, LiveRegion>>,
}

impl ClaimLocationSubscriptions {
    pub(crate) fn reset(&self, region: Region) {
        self.regions.write().insert(region, LiveRegion::default());
    }
    pub(crate) fn invalidate(&self, region: Region) {
        self.regions.write().remove(&region);
    }
    pub(crate) fn finish_initial(&self, region: Region) {
        if let Some(state) = self.regions.write().get_mut(&region) { state.initial_applied = true; }
    }
    pub(crate) fn claims_initial<'a>(&self, region: Region, rows: impl IntoIterator<Item = &'a game_module::module_bindings::ClaimTileState>) {
        if let Some(state) = self.regions.write().get_mut(&region) {
            let rows = rows.into_iter();
            state.claims.reserve(rows.size_hint().0);
            state.coordinates.reserve(rows.size_hint().0);
            for row in rows { state.set_claim(row.entity_id, Some(row.claim_id)); }
            state.finish_claims_initial();
            if state.locations_initial_received { state.missing.shrink_to_fit(); }
        }
    }
    pub(crate) fn claim(&self, region: Region, id: u64, claim: Option<u64>) {
        if let Some(state) = self.regions.write().get_mut(&region) { state.set_claim(id, claim); }
    }
    pub(crate) fn locations_initial<'a>(&self, region: Region, rows: impl IntoIterator<Item = &'a game_module::module_bindings::LocationState>) {
        if let Some(state) = self.regions.write().get_mut(&region) {
            let rows = rows.into_iter();
            state.locations.reserve(rows.size_hint().0);
            for row in rows { state.set_location(row.entity_id, Some((row.dimension, row.x, row.z))); }
            state.locations_initial_received = true;
            // Claims usually arrive first, temporarily placing every tile in
            // this set. Removing entries does not release the backing table.
            state.missing.shrink_to_fit();
        }
    }
    pub(crate) fn location(&self, region: Region, id: u64, location: Option<TileCoordinates>) {
        if let Some(state) = self.regions.write().get_mut(&region) { state.set_location(id, location); }
    }
    pub(crate) fn descriptions_initial<'a>(&self, region: Region, rows: impl IntoIterator<Item = &'a game_module::module_bindings::DimensionDescriptionState>) {
        if let Some(state) = self.regions.write().get_mut(&region) {
            for row in rows { state.descriptions.insert(row.dimension_id, row.dimension_network_entity_id); }
            state.descriptions_initial_received = true;
        }
    }
    pub(crate) fn description(&self, region: Region, dimension: u32, network: Option<u64>) {
        if let Some(state) = self.regions.write().get_mut(&region) {
            if let Some(network) = network { state.descriptions.insert(dimension, network); } else { state.descriptions.remove(&dimension); }
        }
    }
    pub(crate) fn networks_initial<'a>(&self, region: Region, rows: impl IntoIterator<Item = &'a game_module::module_bindings::DimensionNetworkState>) {
        if let Some(state) = self.regions.write().get_mut(&region) {
            for row in rows { state.networks.insert(row.entity_id, row.claim_entity_id); }
            state.networks_initial_received = true;
        }
    }
    pub(crate) fn network(&self, region: Region, id: u64, claim: Option<u64>) {
        if let Some(state) = self.regions.write().get_mut(&region) {
            if let Some(claim) = claim { state.networks.insert(id, claim); } else { state.networks.remove(&id); }
        }
    }
    pub(crate) fn lookup(&self, region: Region, dimension: u32, x: i32, z: i32) -> ClaimAtLocation {
        let regions = self.regions.read();
        let Some(state) = regions.get(&region).filter(|state| state.initial_applied) else { return ClaimAtLocation::Unknown; };
        if dimension != 1 {
            if !state.descriptions_initial_received || !state.networks_initial_received { return ClaimAtLocation::Unknown; }
            return match state.descriptions.get(&dimension).and_then(|id| state.networks.get(id)) {
                Some(0) => ClaimAtLocation::Unclaimed,
                Some(claim) => ClaimAtLocation::Claim(*claim),
                None => ClaimAtLocation::Unknown,
            };
        }
        if !state.claims_initial_received || !state.locations_initial_received || !state.missing.is_empty() { return ClaimAtLocation::Unknown; }
        let (x, z) = mobile_tile(x, z);
        match state.coordinates.get(&(dimension, x, z)) {
            None => ClaimAtLocation::Unclaimed,
            Some(id) => {
                let Some(&claim) = state.claims.get(id) else { return ClaimAtLocation::Unknown; };
                let agrees = state.overlapping.get(&(dimension, x, z)).is_none_or(|others| {
                    others.iter().all(|id| state.claims.get(id) == Some(&claim))
                });
                if agrees { ClaimAtLocation::Claim(claim) } else { ClaimAtLocation::Unknown }
            }
        }
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
    fn interior_lookup_joins_dimension_description_to_network() {
        let index = ClaimLocationSubscriptions::default();
        index.reset(2);
        index.finish_initial(2);
        index.description(2, 15, Some(1000));
        index.network(2, 1000, Some(99));
        assert_eq!(index.lookup(2, 15, 0, 0), ClaimAtLocation::Unknown);
        {
            let mut regions = index.regions.write();
            let region = regions.get_mut(&2).unwrap();
            region.descriptions_initial_received = true;
            region.networks_initial_received = true;
        }
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
