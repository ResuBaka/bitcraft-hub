use crate::AppState;
use crate::world_map::RegionFilter;
use axum::{
    Json,
    extract::{Query, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use entity::shared::Region;
use game_module::module_bindings::*;
use indexmap::IndexMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use spacetimedb_sdk::{Event, Table, TableWithPrimaryKey};
use std::{collections::HashMap, sync::Arc};

const CHUNK: i32 = 96;
const MAX_SPAN: i32 = 8192;
const MAX_CELL: i32 = 96_000;
const MAX_RESULTS: usize = 16_000;
const MAX_PENDING: usize = 2048;
type Cell = (i32, i32);
type TypedCell = (Cell, i32);

struct Paving {
    tile_type_id: i32,
    cell: Option<Cell>,
}

#[derive(Default)]
struct RegionRoads {
    ready: bool,
    subscription_applied: bool,
    paving_loaded: bool,
    locations_loaded: bool,
    paved: HashMap<u64, Paving>,
    chunks: HashMap<Cell, HashMap<TypedCell, u32>>,
    pending: IndexMap<u64, Cell>,
}

fn chunk((x, z): Cell) -> Cell {
    (x.div_euclid(CHUNK), z.div_euclid(CHUNK))
}

fn distance_squared(dx2: i32, dz: i32) -> i64 {
    // Staggered hex rows: twice-X units have one third the squared length
    // of a Z row in the world projection.
    i64::from(dx2).pow(2) + 3 * i64::from(dz).pow(2)
}

fn tile_rank((x, z): Cell, (cx, cz): Cell) -> (i64, Cell) {
    (distance_squared(2 * (x - cx) + (z & 1), z - cz), (x, z))
}

fn keep_nearest(tiles: &mut HashMap<Cell, i32>, center: Cell) -> (i64, Cell) {
    let mut ranks: Vec<_> = tiles.keys().map(|&cell| tile_rank(cell, center)).collect();
    let cutoff = *ranks.select_nth_unstable(MAX_RESULTS - 1).1;
    tiles.retain(|&cell, _| tile_rank(cell, center) <= cutoff);
    cutoff
}

impl RegionRoads {
    fn finish_initial(&mut self) {
        if !self.ready && self.subscription_applied && self.paving_loaded && self.locations_loaded {
            self.pending = IndexMap::new();
            self.ready = true;
        }
    }

    fn add_cell(&mut self, cell: Cell, tile_type_id: i32) {
        *self
            .chunks
            .entry(chunk(cell))
            .or_default()
            .entry((cell, tile_type_id))
            .or_default() += 1;
    }

    fn remove_cell(&mut self, cell: Cell, tile_type_id: i32) {
        let key = chunk(cell);
        if let Some(counts) = self.chunks.get_mut(&key) {
            let typed_cell = (cell, tile_type_id);
            if let Some(count) = counts.get_mut(&typed_cell) {
                *count -= 1;
                if *count == 0 {
                    counts.remove(&typed_cell);
                }
            }
            if counts.is_empty() {
                self.chunks.remove(&key);
            }
        }
    }

    fn set_paved(&mut self, id: u64, tile_type_id: Option<i32>) {
        if let Some(tile_type_id) = tile_type_id {
            let paving = self.paved.entry(id).or_insert(Paving {
                tile_type_id,
                cell: None,
            });
            let previous = std::mem::replace(&mut paving.tile_type_id, tile_type_id);
            if previous != tile_type_id
                && let Some(cell) = paving.cell
            {
                self.remove_cell(cell, previous);
                self.add_cell(cell, tile_type_id);
            }
            if let Some(cell) = self.pending.swap_remove(&id) {
                self.set_location(id, Some(cell));
            }
        } else {
            self.pending.swap_remove(&id);
            if let Some(Paving {
                tile_type_id,
                cell: Some(cell),
            }) = self.paved.remove(&id)
            {
                self.remove_cell(cell, tile_type_id);
            }
        }
    }

    fn set_location(&mut self, id: u64, location: Option<Cell>) {
        if let Some(paving) = self.paved.get_mut(&id) {
            let before = std::mem::replace(&mut paving.cell, location);
            let tile_type_id = paving.tile_type_id;
            if before == location {
                return;
            }
            if let Some(before) = before {
                self.remove_cell(before, tile_type_id);
            }
            if let Some(cell) = location {
                self.add_cell(cell, tile_type_id);
            }
        } else if let Some(cell) = location {
            if !self.paving_loaded || self.ready {
                // A location callback can precede its paving row. During live
                // updates retain only a small rolling window of unmatched rows.
                self.pending.insert(id, cell);
                if self.ready && self.pending.len() > MAX_PENDING {
                    self.pending.shift_remove_index(0);
                }
            }
        } else {
            self.pending.swap_remove(&id);
        }
    }
}

#[derive(Default)]
pub(crate) struct AtlasRoads {
    regions: RwLock<HashMap<Region, RegionRoads>>,
    tiers: RwLock<HashMap<i32, i32>>,
}

impl AtlasRoads {
    pub(super) fn replace_tiers(&self, tiers: impl IntoIterator<Item = (i32, i32)>) {
        *self.tiers.write() = tiers.into_iter().collect();
    }

    pub(super) fn set_tier(&self, id: i32, tier: Option<i32>) {
        let mut tiers = self.tiers.write();
        if let Some(tier) = tier {
            tiers.insert(id, tier);
        } else {
            tiers.remove(&id);
        }
    }

    fn describe(&self, mut roads: Roads) -> Roads {
        let tiers = self.tiers.read();
        roads.tiers = roads
            .runs
            .iter()
            .filter_map(|&(_, _, _, id)| tiers.get(&id).map(|&tier| (id, tier)))
            .collect();
        roads
    }

    fn update(&self, region: Region, f: impl FnOnce(&mut RegionRoads)) {
        if let Some(roads) = self.regions.write().get_mut(&region) {
            f(roads);
        }
    }

    pub(crate) fn ready(&self, region: Region) {
        self.update(region, |r| {
            r.subscription_applied = true;
            r.finish_initial();
        });
    }

    pub(crate) fn invalidate(&self, region: Region) {
        self.regions.write().remove(&region);
    }

    fn viewport_cells(
        &self,
        bounds: Viewport,
        center: Option<Cell>,
        filter: &RegionFilter,
    ) -> (HashMap<Cell, i32>, bool) {
        let min = chunk((bounds.min_x, bounds.min_z));
        let max = chunk((bounds.max_x, bounds.max_z));
        let cells = (min.0..=max.0).flat_map(|x| (min.1..=max.1).map(move |z| (x, z)));
        let mut chunks: Vec<_> = cells
            .map(|cell| (center.map_or(0, |c| bounds.chunk_distance(cell, c)), cell))
            .collect();
        if center.is_some() {
            chunks.sort_unstable();
        }
        let regions = self.regions.read();
        let ready: Vec<_> = regions
            .iter()
            .filter_map(|(id, r)| (filter.includes(*id) && r.ready).then_some(r))
            .collect();
        let mut tiles = HashMap::<Cell, i32>::new();
        let mut cutoff: Option<(i64, Cell)> = None;
        for (distance, chunk) in chunks {
            // The cutoff can only get closer. Chunks beyond it can't
            // contribute, but ties still need to be visited.
            if cutoff.is_some_and(|(d, _)| distance > d) {
                break;
            }
            for region in &ready {
                let Some(cells) = region.chunks.get(&chunk) else {
                    continue;
                };
                for &(cell, tile_type_id) in cells.keys() {
                    if !bounds.contains(cell)
                        || center
                            .zip(cutoff)
                            .is_some_and(|(center, limit)| tile_rank(cell, center) > limit)
                    {
                        continue;
                    }
                    // Pick consistently if two rows briefly overlap while replacing paving.
                    tiles
                        .entry(cell)
                        .and_modify(|id| *id = (*id).min(tile_type_id))
                        .or_insert(tile_type_id);
                    if center.is_none() && tiles.len() > MAX_RESULTS {
                        // The padded result will be discarded in favor of the viewport.
                        return (tiles, true);
                    }
                }
            }
            if let Some(center) = center
                && tiles.len() > MAX_RESULTS
                && (cutoff.is_none() || tiles.len() >= 2 * MAX_RESULTS)
            {
                cutoff = Some(keep_nearest(&mut tiles, center));
            }
        }
        drop(regions);
        if let Some(center) = center
            && tiles.len() > MAX_RESULTS
        {
            cutoff = Some(keep_nearest(&mut tiles, center));
        }
        (tiles, cutoff.is_some())
    }

    fn viewport_overview(&self, bounds: Viewport, filter: &RegionFilter) -> Roads {
        let mut scale = 2;
        let mut buckets = HashMap::<Cell, i32>::new();
        let min = chunk((bounds.min_x, bounds.min_z));
        let max = chunk((bounds.max_x, bounds.max_z));
        let regions = self.regions.read();
        for (_, region) in regions
            .iter()
            .filter(|(id, r)| filter.includes(**id) && r.ready)
        {
            for x in min.0..=max.0 {
                for z in min.1..=max.1 {
                    let Some(cells) = region.chunks.get(&(x, z)) else {
                        continue;
                    };
                    for &((x, z), id) in cells.keys() {
                        if !bounds.contains((x, z)) {
                            continue;
                        }
                        put_overview_bucket(&mut buckets, (x / scale, z / scale), id);
                        while buckets.len() > MAX_RESULTS {
                            // Fold existing buckets instead of revisiting tiles.
                            // The smallest material ID wins consistently at every scale.
                            let mut coarser = HashMap::new();
                            for ((x, z), id) in buckets.drain() {
                                put_overview_bucket(&mut coarser, (x / 2, z / 2), id);
                            }
                            buckets = coarser;
                            scale *= 2;
                        }
                    }
                }
            }
        }
        drop(regions);
        let mut roads = Roads::new(buckets, false);
        roads.scale = scale;
        roads
    }

    #[cfg(test)]
    fn viewport(&self, bounds: Viewport, visible: Viewport, center: Cell, overview: bool) -> Roads {
        self.viewport_for_regions(bounds, visible, center, overview, &RegionFilter::default())
    }

    fn viewport_for_regions(
        &self,
        bounds: Viewport,
        visible: Viewport,
        center: Cell,
        overview: bool,
        filter: &RegionFilter,
    ) -> Roads {
        let (mut tiles, mut truncated) = self.viewport_cells(
            bounds,
            (!overview && visible == bounds).then_some(center),
            filter,
        );
        let visible_only = truncated && visible != bounds;
        if visible_only {
            drop(tiles);
            (tiles, truncated) =
                self.viewport_cells(visible, (!overview).then_some(center), filter);
        }
        let mut roads = if truncated && overview {
            drop(tiles);
            self.viewport_overview(visible, filter)
        } else {
            Roads::new(tiles, truncated)
        };
        roads.visible_only = visible_only;
        roads
    }
}

fn put_overview_bucket(buckets: &mut HashMap<Cell, i32>, cell: Cell, id: i32) {
    buckets
        .entry(cell)
        .and_modify(|old| *old = (*old).min(id))
        .or_insert(id);
}

fn location(row: &LocationState) -> Option<Cell> {
    (row.dimension == 1).then_some((row.x, row.z))
}

fn standalone_type(row: &PavedTileState) -> Option<i32> {
    (row.related_entity_id == 0).then_some(row.tile_type_id)
}

pub(super) fn listen(ctx: &DbConnection, region: Region, atlas: Arc<AtlasRoads>) {
    atlas.regions.write().insert(region, RegionRoads::default());
    let paving = ctx.db.paved_tile_state();
    let target = atlas.clone();
    paving.on_initial_rows(move |_, rows| {
        target.update(region, |r| {
            r.paved.reserve(rows.iter().size_hint().0);
            for row in rows.iter() {
                r.set_paved(row.entity_id, standalone_type(row));
            }
            r.paving_loaded = true;
            r.pending = IndexMap::new();
            r.finish_initial();
        })
    });
    let target = atlas.clone();
    paving.on_insert(move |ctx, row| {
        if !matches!(ctx.event, Event::SubscribeApplied) {
            target.update(region, |r| r.set_paved(row.entity_id, standalone_type(row)));
        }
    });
    let target = atlas.clone();
    paving.on_update(move |_, _, row| {
        target.update(region, |r| r.set_paved(row.entity_id, standalone_type(row)))
    });
    let target = atlas.clone();
    paving.on_delete(move |_, row| target.update(region, |r| r.set_paved(row.entity_id, None)));

    let locations = ctx.db.location_state();
    let target = atlas.clone();
    locations.on_initial_rows(move |_, rows| {
        target.update(region, |r| {
            for row in rows.iter() {
                if let Some(cell) = location(row) {
                    r.set_location(row.entity_id, Some(cell));
                }
            }
            r.locations_loaded = true;
            r.finish_initial();
        })
    });
    let target = atlas.clone();
    locations.on_insert(move |ctx, row| {
        if !matches!(ctx.event, Event::SubscribeApplied) {
            target.update(region, |r| r.set_location(row.entity_id, location(row)));
        }
    });
    let target = atlas.clone();
    locations.on_update(move |_, _, row| {
        target.update(region, |r| r.set_location(row.entity_id, location(row)))
    });
    locations
        .on_delete(move |_, row| atlas.update(region, |r| r.set_location(row.entity_id, None)));
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Viewport {
    min_x: i32,
    min_z: i32,
    max_x: i32,
    max_z: i32,
}

impl Viewport {
    fn valid(self) -> bool {
        (0..=MAX_CELL).contains(&self.min_x)
            && (self.min_x..=MAX_CELL).contains(&self.max_x)
            && (0..=MAX_CELL).contains(&self.min_z)
            && (self.min_z..=MAX_CELL).contains(&self.max_z)
            && self.max_x - self.min_x < MAX_SPAN
            && self.max_z - self.min_z < MAX_SPAN
    }

    fn contains(self, (x, z): Cell) -> bool {
        (self.min_x..=self.max_x).contains(&x) && (self.min_z..=self.max_z).contains(&z)
    }

    fn center(self) -> Cell {
        ((self.min_x + self.max_x) / 2, (self.min_z + self.max_z) / 2)
    }

    fn chunk_distance(self, (x, z): Cell, (cx, cz): Cell) -> i64 {
        let west = self.min_x.max(x * CHUNK);
        let east = self.max_x.min((x + 1) * CHUNK - 1);
        let south = self.min_z.max(z * CHUNK);
        let north = self.max_z.min((z + 1) * CHUNK - 1);
        // Include both odd- and even-row X extents for a safe lower bound.
        let dx2 = (2 * cx).clamp(2 * west, 2 * east + 1) - 2 * cx;
        let dz = cz.clamp(south, north) - cz;
        distance_squared(dx2, dz)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RoadQuery {
    min_x: i32,
    min_z: i32,
    max_x: i32,
    max_z: i32,
    visible_min_x: Option<i32>,
    visible_min_z: Option<i32>,
    visible_max_x: Option<i32>,
    visible_max_z: Option<i32>,
    center_x: Option<i32>,
    center_z: Option<i32>,
    #[serde(default)]
    overview: bool,
    regions: Option<String>,
}

impl RoadQuery {
    fn viewports(&self) -> Option<(Viewport, Viewport, Cell)> {
        let bounds = Viewport {
            min_x: self.min_x,
            min_z: self.min_z,
            max_x: self.max_x,
            max_z: self.max_z,
        };
        if !bounds.valid() {
            return None;
        }
        let visible = match (
            self.visible_min_x,
            self.visible_min_z,
            self.visible_max_x,
            self.visible_max_z,
        ) {
            (None, None, None, None) => bounds,
            (Some(min_x), Some(min_z), Some(max_x), Some(max_z))
                if min_x <= max_x
                    && min_z <= max_z
                    && bounds.contains((min_x, min_z))
                    && bounds.contains((max_x, max_z)) =>
            {
                Viewport {
                    min_x,
                    min_z,
                    max_x,
                    max_z,
                }
            }
            _ => return None,
        };
        let center = match (self.center_x, self.center_z) {
            (None, None) => visible.center(),
            (Some(x), Some(z)) if visible.contains((x, z)) => (x, z),
            _ => return None,
        };
        Some((bounds, visible, center))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Roads {
    // Horizontal same-material runs: [first X, last X (inclusive), Z, tile type ID].
    runs: Vec<(i32, i32, i32, i32)>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    tiers: HashMap<i32, i32>,
    truncated: bool,
    visible_only: bool,
    scale: i32,
}

impl Roads {
    fn new(cells: HashMap<Cell, i32>, truncated: bool) -> Self {
        let mut cells: Vec<_> = cells.into_iter().map(|((x, z), id)| (z, x, id)).collect();
        cells.sort_unstable();
        let mut runs: Vec<(i32, i32, i32, i32)> = Vec::new();
        for (z, x, tile_type_id) in cells.into_iter().take(MAX_RESULTS) {
            if let Some(last) = runs.last_mut()
                && last.2 == z
                && last.1 + 1 == x
                && last.3 == tile_type_id
            {
                last.1 = x;
            } else {
                runs.push((x, x, z, tile_type_id));
            }
        }
        Self {
            runs,
            tiers: HashMap::new(),
            truncated,
            visible_only: false,
            scale: 1,
        }
    }
}

pub(crate) async fn get(
    State(state): State<AppState>,
    Query(query): Query<RoadQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let (bounds, visible, center) = query.viewports().ok_or(StatusCode::BAD_REQUEST)?;
    let filter = RegionFilter::parse(query.regions.as_deref())?;
    let roads = tokio::task::spawn_blocking(move || {
        let atlas = &state.atlas_roads;
        atlas.describe(atlas.viewport_for_regions(bounds, visible, center, query.overview, &filter))
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(([(header::CACHE_CONTROL, "public, max-age=15")], Json(roads)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_prefetch_falls_back_to_the_visible_area_without_truncating_it() {
        let atlas = AtlasRoads::default();
        let mut region = RegionRoads {
            ready: true,
            ..Default::default()
        };
        for x in 0..200 {
            for z in 0..100 {
                region.add_cell((x, z), 11);
            }
        }
        atlas.regions.write().insert(7, region);
        let bounds = Viewport {
            min_x: 0,
            min_z: 0,
            max_x: 199,
            max_z: 99,
        };
        let visible = Viewport {
            min_x: 100,
            min_z: 99,
            ..bounds
        };
        let padded = atlas.viewport(bounds, bounds, bounds.center(), false);
        assert!(padded.truncated);
        assert!(!padded.visible_only);
        let result = atlas.viewport(bounds, visible, visible.center(), true);
        assert!(result.visible_only);
        assert!(!result.truncated);
        assert_eq!(result.runs, [(100, 199, 99, 11)]);
    }

    #[test]
    fn capped_paving_keeps_the_nearest_tiles_across_chunks() {
        let atlas = AtlasRoads::default();
        let mut region = RegionRoads {
            ready: true,
            ..Default::default()
        };
        let mut cells = Vec::new();
        for x in 0..320 {
            for z in 0..175 {
                if (x + 5 * z) % 7 == 0 {
                    continue;
                }
                cells.push((x, z));
                region.add_cell((x, z), 11);
            }
        }
        atlas.regions.write().insert(7, region);
        let bounds = Viewport {
            min_x: 10,
            min_z: 8,
            max_x: 299,
            max_z: 155,
        };
        let center = (246, 130);
        let roads = atlas.viewport(bounds, bounds, center, false);
        assert!(roads.truncated);
        let mut actual: Vec<_> = roads
            .runs
            .into_iter()
            .flat_map(|(west, east, z, _)| (west..=east).map(move |x| (x, z)))
            .collect();
        let mut expected = cells
            .into_iter()
            .filter(|&c| bounds.contains(c))
            .collect::<Vec<_>>();
        expected.sort_unstable_by_key(|&c| tile_rank(c, center));
        expected.truncate(MAX_RESULTS);
        expected.sort_unstable();
        actual.sort_unstable();
        assert_eq!(actual, expected);
    }

    #[test]
    fn overview_keeps_distant_roads_when_a_dense_center_exceeds_the_tile_limit() {
        let atlas = AtlasRoads::default();
        let mut region = RegionRoads {
            ready: true,
            ..Default::default()
        };
        for x in 100..400 {
            for z in 100..400 {
                region.add_cell((x, z), 11);
            }
        }
        let distant = [(5, 6), (703, 570)];
        for cell in distant {
            region.add_cell(cell, 7);
        }
        atlas.regions.write().insert(7, region);
        let bounds = Viewport {
            min_x: 0,
            min_z: 0,
            max_x: 749,
            max_z: 599,
        };
        let roads = atlas.viewport(bounds, bounds, bounds.center(), true);
        assert_eq!(roads.scale, 4);
        assert!(!roads.truncated);
        assert!(
            roads
                .runs
                .iter()
                .map(|(x1, x2, _, _)| (x2 - x1 + 1) as usize)
                .sum::<usize>()
                <= MAX_RESULTS
        );
        for (x, z) in distant {
            let (bx, bz) = (x / roads.scale, z / roads.scale);
            assert!(
                roads
                    .runs
                    .iter()
                    .any(|&(west, east, z, id)| (west..=east).contains(&bx) && z == bz && id == 7)
            );
        }
    }

    #[test]
    fn visible_area_must_be_complete_and_inside_the_prefetch_bounds() {
        let viewports = |max_x, max_z: Option<i32>| {
            serde_json::from_value::<RoadQuery>(serde_json::json!({
                "minX": 0, "minZ": 0, "maxX": 180, "maxZ": 180,
                "visibleMinX": 100, "visibleMinZ": 100,
                "visibleMaxX": max_x, "visibleMaxZ": max_z,
            }))
            .unwrap()
            .viewports()
        };
        assert!(viewports(160, Some(160)).is_some());
        assert!(viewports(190, Some(160)).is_none());
        assert!(viewports(160, None).is_none());
    }

    #[test]
    fn sends_only_tiers_for_tile_types_in_the_response() {
        let atlas = AtlasRoads::default();
        atlas.replace_tiers([(7, 2), (-21, 9), (30, 10)]);
        let runs = || {
            Roads::new(
                HashMap::from([((1, 2), 7), ((2, 2), -21), ((3, 2), 11)]),
                false,
            )
        };
        let described = atlas.describe(runs());
        assert_eq!(described.tiers, HashMap::from([(7, 2), (-21, 9)]));
        atlas.set_tier(7, Some(4));
        atlas.set_tier(-21, None);
        assert_eq!(atlas.describe(runs()).tiers, HashMap::from([(7, 4)]));
    }

    #[test]
    fn joins_both_orders_and_removes_pavement_without_erasing_overlapping_cells() {
        let atlas = AtlasRoads::default();
        atlas.regions.write().insert(7, RegionRoads::default());
        atlas.ready(7); // SDK calls on_applied before table callbacks.
        atlas.update(7, |r| {
            r.set_location(1, Some((96, 5)));
            r.set_paved(1, Some(11));
            r.set_paved(2, Some(11));
            r.set_location(2, Some((95, 5)));
            r.set_paved(3, Some(15));
            r.set_location(3, Some((95, 5)));
            r.set_location(4, Some((97, 5))); // location without a paving row
            r.set_paved(5, Some(11));
            r.set_location(5, Some((90, 5))); // outside the query
        });
        let bounds = Viewport {
            min_x: 95,
            max_x: 97,
            min_z: 0,
            max_z: 25,
        };
        assert!(
            atlas
                .viewport(bounds, bounds, bounds.center(), false)
                .runs
                .is_empty()
        );
        atlas.update(7, |r| {
            r.paving_loaded = true;
            r.locations_loaded = true;
            r.finish_initial();
        });
        assert_eq!(
            atlas.viewport(bounds, bounds, bounds.center(), false).runs,
            [(95, 96, 5, 11)]
        );
        // Updating an existing tile's material splits an otherwise contiguous run.
        atlas.update(7, |r| r.set_paved(1, Some(17)));
        assert_eq!(
            atlas.viewport(bounds, bounds, bounds.center(), false).runs,
            [(95, 95, 5, 11), (96, 96, 5, 17)]
        );
        atlas.update(7, |r| {
            r.set_location(2, None);
        });
        assert_eq!(
            atlas.viewport(bounds, bounds, bounds.center(), false).runs,
            [(95, 95, 5, 15), (96, 96, 5, 17)]
        );
        atlas.update(7, |r| {
            r.set_paved(3, None);
            r.set_location(6, Some((97, 5)));
            r.set_paved(6, Some(-91));
            r.set_paved(1, None);
        });
        assert_eq!(
            atlas.viewport(bounds, bounds, bounds.center(), false).runs,
            [(97, 97, 5, -91)]
        );
        atlas.invalidate(7);
        assert!(
            atlas
                .viewport(bounds, bounds, bounds.center(), false)
                .runs
                .is_empty()
        );
    }
}
