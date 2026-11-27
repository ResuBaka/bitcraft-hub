use anyhow::{Context, Result, ensure};
use game_module::module_bindings::{TerrainChunkState, WorldRegionState};
use image::{Rgba, RgbaImage, imageops};
use rayon::prelude::*;
use serde::Serialize;
use std::{
    collections::{BTreeSet, HashMap},
    path::{Path, PathBuf},
};

const CHUNK: i32 = 32;
const CELLS: usize = (CHUNK * CHUNK) as usize;
const TILE: u32 = 512;
const HEX_X: f64 = 10.0 * 1.732_050_807_568_877_2;
const HEX_Z: f64 = 15.0;
pub(super) const MAX_ZOOM: u8 = 10;
// In-game ocean color, not the charcoal fog-of-war.
const OCEAN: [u8; 3] = [0x28, 0x2f, 0x41];
const OCEAN_BACKGROUND: Rgba<u8> = Rgba([OCEAN[0], OCEAN[1], OCEAN[2], 255]);
const WATER_FEATHER_CELLS: usize = 24;
const LAND_SLOPE_STRENGTH: f32 = 0.3;
// Indices are BitCraft's Biome values. Approximate the revealed in-game
// map palette rather than the charcoal colors of unexplored tiles.
const BIOMES: [[u8; 3]; 19] = [
    [75, 80, 68],       // Dev
    [0x41, 0x4a, 0x32], // Calm Forest
    [0x42, 0x49, 0x38], // Pine Woods
    [104, 109, 108],    // Snowy Peaks
    [0x62, 0x62, 0x4a], // Breezy Grasslands
    [0x45, 0x3e, 0x2f], // Autumn Forest
    [72, 78, 66],       // Misty Tundra
    [100, 88, 65],      // Desert Wasteland
    [63, 68, 46],       // Swamp
    [0x4c, 0x4a, 0x56], // Rocky Garden
    OCEAN,              // Open Ocean
    [0x65, 0x6d, 0x53], // Safe Meadows (Starter Town)
    [66, 65, 72],       // Cave
    [53, 65, 48],       // Jungle
    [63, 75, 55],       // Sapwoods
    [103, 94, 73],      // Deserted Beach
    [59, 72, 51],       // Tropical Canopy
    [66, 59, 57],       // Volcanic Crag
    OCEAN,              // Uncharted Ocean
];

type Coord = (i32, i32);
type TileCoord = (u32, u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Layout {
    origin: Coord,
    region_size: Coord,
    side: i32,
}

impl Layout {
    fn from_region(region: &WorldRegionState) -> Result<Self> {
        let side = i32::from(region.region_count_sqrt);
        let index = i32::from(region.region_index) - 1;
        let count = i32::from(region.region_count);
        ensure!(
            side > 0 && side * side == count && (0..count).contains(&index),
            "Invalid world region numbering"
        );
        let width = i32::from(region.region_width_chunks);
        let height = i32::from(region.region_height_chunks);
        let origin = (
            i32::from(region.region_min_chunk_x) - index % side * width,
            i32::from(region.region_min_chunk_z) - index / side * height,
        );
        ensure!(
            width > 0
                && height > 0
                && origin.0 >= 0
                && origin.1 >= 0
                && origin.0 + width * side <= 1000
                && origin.1 + height * side <= 1000,
            "World exceeds the Overworld chunk coordinate bounds"
        );
        Ok(Self {
            origin,
            region_size: (width, height),
            side,
        })
    }

    fn size(self) -> Coord {
        (
            self.region_size.0 * self.side,
            self.region_size.1 * self.side,
        )
    }

    fn region(self, id: u8, data: Option<&RegionData>) -> Region {
        let index = i32::from(id) - 1;
        Region {
            id,
            x: self.origin.0 + index % self.side * self.region_size.0,
            z: self.origin.1 + index / self.side * self.region_size.1,
            width: self.region_size.0,
            height: self.region_size.1,
            chunks: data.map_or(0, |data| data.chunks),
            active: data.is_some(),
            name: data.and_then(|data| data.name.clone()),
        }
    }
}

#[derive(Serialize)]
struct Region {
    id: u8,
    x: i32,
    z: i32,
    width: i32,
    height: i32,
    chunks: usize,
    active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

struct RegionData {
    chunks: usize,
    name: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Manifest {
    version: String,
    label: &'static str,
    dimension: u32,
    extent: [f64; 4],
    tile_size: u32,
    background_color: String,
    resolutions: Vec<f64>,
    tiles: Vec<Vec<u32>>,
    regions: Vec<Region>,
}

struct Chunk {
    biomes: Vec<u8>,
    elevations: Vec<i16>,
    water: Vec<i16>,
}

#[derive(Default)]
pub(super) struct World {
    layout: Option<Layout>,
    chunks: HashMap<Coord, Chunk>,
    regions: HashMap<u8, RegionData>,
}

impl World {
    pub(super) fn insert(
        &mut self,
        region: WorldRegionState,
        name: Option<String>,
        terrain: Vec<TerrainChunkState>,
    ) -> Result<()> {
        let layout = Layout::from_region(&region)?;
        let expected = self.layout.get_or_insert(layout);
        ensure!(
            *expected == layout,
            "World region {} does not match other regions",
            region.region_index
        );
        let bounds = layout.region(region.region_index, None);
        ensure!(
            self.regions
                .insert(
                    bounds.id,
                    RegionData {
                        chunks: terrain.len(),
                        name
                    }
                )
                .is_none(),
            "Duplicate region {}",
            bounds.id
        );
        for chunk in terrain {
            let (x, z) = (chunk.chunk_x, chunk.chunk_z);
            ensure!(
                chunk.dimension == 1
                    && (bounds.x..bounds.x + bounds.width).contains(&x)
                    && (bounds.z..bounds.z + bounds.height).contains(&z),
                "Terrain chunk ({x}, {z}) is outside Overworld region {}",
                bounds.id
            );
            ensure!(
                chunk.biomes.len() == CELLS
                    && chunk.elevations.len() == CELLS
                    && chunk.water_levels.len() == CELLS,
                "Terrain chunk ({x}, {z}) does not contain {CELLS} cells"
            );
            let row = Chunk {
                biomes: chunk
                    .biomes
                    .into_iter()
                    .map(|packed| packed as u8)
                    .collect(),
                elevations: chunk.elevations,
                water: chunk.water_levels,
            };
            ensure!(
                self.chunks.insert((x, z), row).is_none(),
                "Duplicate terrain chunk ({x}, {z})"
            );
        }
        Ok(())
    }

    pub(super) fn build(self, root: &Path, version: String, quality: u8) -> Result<Manifest> {
        let layout = self
            .layout
            .context("No regional SpacetimeDB databases are configured")?;
        ensure!(!self.chunks.is_empty(), "No Overworld terrain was returned");
        let (width, height) = layout.size();
        let west = f64::from(layout.origin.0 * CHUNK - 1) * HEX_X;
        let north = f64::from((layout.origin.1 + height) * CHUNK) * HEX_Z;
        let span =
            (f64::from(width * CHUNK + 2) * HEX_X).max(f64::from(height * CHUNK + 2) * HEX_Z);
        let extent = [west, north - span, west + span, north];
        let max_z = (span / (f64::from(TILE) * 4.5))
            .log2()
            .ceil()
            .clamp(0.0, f64::from(MAX_ZOOM)) as u8;
        let resolutions = (0..=max_z)
            .map(|z| span / f64::from(TILE << z))
            .collect::<Vec<_>>();
        let tile_span = span / f64::from(1u32 << max_z);
        let limit = (1i32 << max_z) - 1;
        let mut candidates = BTreeSet::new();
        for &(x, z) in self.chunks.keys() {
            // Pad the nominal rectangle to include staggered hex edges.
            let left = f64::from(x * CHUNK - 1) * HEX_X;
            let right = f64::from((x + 1) * CHUNK + 1) * HEX_X;
            let bottom = f64::from(z * CHUNK - 1) * HEX_Z;
            let top = f64::from((z + 1) * CHUNK + 1) * HEX_Z;
            let tile =
                |offset: f64| (offset / tile_span).floor().clamp(0.0, f64::from(limit)) as u32;
            for tx in tile(left - west)..=tile(right - west) {
                for ty in tile(north - top)..=tile(north - bottom) {
                    candidates.insert((tx, ty));
                }
            }
        }
        let regions = (1..=(layout.side * layout.side) as u8)
            .map(|id| layout.region(id, self.regions.get(&id)))
            .collect();
        let colors = ColorWorld::from_world(self, layout);
        let mut children = candidates
            .into_par_iter()
            .filter_map(|(x, y)| {
                let image = raster_tile((x, y), extent, resolutions[usize::from(max_z)], &colors)?;
                Some(save(root, max_z, (x, y), &image).map(|()| (x, y)))
            })
            .collect::<Result<BTreeSet<_>>>()?;
        drop(colors);
        let mut tiles = vec![Vec::new(); usize::from(max_z) + 1];
        for z in (0..=max_z).rev() {
            let side = 1 << z;
            let level = &mut tiles[usize::from(z)];
            level.extend(children.iter().map(|&(x, y)| y * side + x));
            level.sort_unstable();
            if z == 0 {
                break;
            }
            let parents = children
                .iter()
                .map(|&(x, y)| (x / 2, y / 2))
                .collect::<BTreeSet<_>>();
            parents.par_iter().try_for_each(|&(x, y)| -> Result<()> {
                let mut image = RgbaImage::from_pixel(TILE * 2, TILE * 2, OCEAN_BACKGROUND);
                for dy in 0..2 {
                    for dx in 0..2 {
                        let coord = (x * 2 + dx, y * 2 + dy);
                        if children.contains(&coord) {
                            let tile = image::open(tile_path(root, z, coord))?.into_rgba8();
                            imageops::replace(
                                &mut image,
                                &tile,
                                i64::from(dx * TILE),
                                i64::from(dy * TILE),
                            );
                        }
                    }
                }
                save(
                    root,
                    z - 1,
                    (x, y),
                    &imageops::resize(&image, TILE, TILE, imageops::FilterType::Triangle),
                )
            })?;
            children = parents;
        }
        // Build every parent from lossless pixels before encoding delivery
        // tiles, so compression artifacts don't accumulate at wider zooms.
        tiles.par_iter().enumerate().try_for_each(|(z, keys)| {
            let side = 1u32 << z;
            keys.par_iter().try_for_each(|&key| {
                let path = tile_path(root, z as u8, (key % side, key / side));
                encode_delivery_tile(&path, quality)
                    .with_context(|| format!("Could not compress {}", path.display()))
            })
        })?;
        tracing::info!(
            tiles = tiles.iter().map(Vec::len).sum::<usize>(),
            "Built world atlas"
        );
        Ok(Manifest {
            version,
            label: "SpacetimeDB terrain snapshot",
            dimension: 1,
            extent,
            tile_size: TILE,
            background_color: format!(
                "#{:02x}{:02x}{:02x}",
                OCEAN_BACKGROUND[0], OCEAN_BACKGROUND[1], OCEAN_BACKGROUND[2]
            ),
            resolutions,
            tiles,
            regions,
        })
    }
}

struct ColorWorld {
    origin: Coord,
    size: Coord,
    chunks: Vec<Option<Vec<Rgba<u8>>>>,
}

impl ColorWorld {
    fn from_world(world: World, layout: Layout) -> Self {
        let size = layout.size();
        let mut chunks = vec![None; (size.0 * size.1) as usize];
        chunks.par_iter_mut().enumerate().for_each(|(index, slot)| {
            let x = layout.origin.0 + index as i32 % size.0;
            let z = layout.origin.1 + index as i32 / size.0;
            let Some(chunk) = world.chunks.get(&(x, z)) else {
                return;
            };
            let west = world.chunks.get(&(x - 1, z));
            let east = world.chunks.get(&(x + 1, z));
            let south = world.chunks.get(&(x, z - 1));
            let north = world.chunks.get(&(x, z + 1));
            let edges = [
                west.is_none(),
                east.is_none(),
                south.is_none(),
                north.is_none(),
            ];
            let diagonals = [(-1, -1), (1, -1), (-1, 1), (1, 1)]
                .map(|(dx, dz)| !world.chunks.contains_key(&(x + dx, z + dz)));
            let borders_gap = edges.iter().chain(&diagonals).any(|&missing| missing);
            let stride = CHUNK as usize;
            *slot = Some(
                (0..CELLS)
                    .map(|i| {
                        let elevation = chunk.elevations[i];
                        let depth = i32::from(chunk.water[i]) - i32::from(elevation);
                        if depth > 0 {
                            let color = water_color(depth);
                            return if borders_gap {
                                feather_water(color, edges, diagonals, i % stride, i / stride)
                            } else {
                                color
                            };
                        }
                        let lx = i % stride;
                        let lz = i / stride;
                        let row_start = lz * stride;
                        let west = if lx > 0 {
                            chunk.elevations[i - 1]
                        } else {
                            west.map_or(elevation, |c| c.elevations[row_start + stride - 1])
                        };
                        let east = if lx + 1 < stride {
                            chunk.elevations[i + 1]
                        } else {
                            east.map_or(elevation, |c| c.elevations[row_start])
                        };
                        let south = if lz > 0 {
                            chunk.elevations[i - stride]
                        } else {
                            south.map_or(elevation, |c| c.elevations[CELLS - stride + lx])
                        };
                        let north = if lz + 1 < stride {
                            chunk.elevations[i + stride]
                        } else {
                            north.map_or(elevation, |c| c.elevations[lx])
                        };
                        let shade = (i32::from(east) - i32::from(west) - i32::from(north)
                            + i32::from(south))
                        .clamp(-18, 18) as f32
                            * LAND_SLOPE_STRENGTH;
                        let relief = (f32::from(elevation) * 0.14).clamp(0., 10.);
                        let color = BIOMES
                            .get(usize::from(chunk.biomes[i]))
                            .copied()
                            .unwrap_or(BIOMES[0]);
                        tint(color, [relief; 3], shade)
                    })
                    .collect(),
            );
        });
        Self {
            origin: layout.origin,
            size,
            chunks,
        }
    }

    fn sample(&self, x: i32, z: i32) -> Option<Rgba<u8>> {
        let x = x - self.origin.0 * CHUNK;
        let z = z - self.origin.1 * CHUNK;
        if x < 0 || z < 0 || x >= self.size.0 * CHUNK || z >= self.size.1 * CHUNK {
            return None;
        }
        let chunk = self.chunks[(z / CHUNK * self.size.0 + x / CHUNK) as usize].as_ref()?;
        Some(chunk[(z % CHUNK * CHUNK + x % CHUNK) as usize])
    }
}

fn tint(base: [u8; 3], tint: [f32; 3], shade: f32) -> Rgba<u8> {
    let channel = |i: usize| (f32::from(base[i]) + tint[i] + shade).clamp(0., 255.) as u8;
    Rgba([channel(0), channel(1), channel(2), 255])
}

fn water_color(depth: i32) -> Rgba<u8> {
    let shallows = (-(depth as f32) / 9.).exp();
    tint(OCEAN, [5. * shallows, 5. * shallows, 8. * shallows], 0.)
}

fn feather_water(
    mut color: Rgba<u8>,
    edges: [bool; 4],
    diagonals: [bool; 4],
    x: usize,
    z: usize,
) -> Rgba<u8> {
    let east = CHUNK as usize - 1 - x;
    let north = CHUNK as usize - 1 - z;
    let [w, e, s, n] = [x * x, east * east, z * z, north * north];
    let mut distance_sq = WATER_FEATHER_CELLS * WATER_FEATHER_CELLS;
    for (missing, distance) in
        edges
            .into_iter()
            .chain(diagonals)
            .zip([w, e, s, n, w + s, e + s, w + n, e + n])
    {
        if missing {
            distance_sq = distance_sq.min(distance);
        }
    }
    if distance_sq < WATER_FEATHER_CELLS * WATER_FEATHER_CELLS {
        let t = (distance_sq as f32).sqrt() / WATER_FEATHER_CELLS as f32;
        let weight = t * t * (3.0 - 2.0 * t);
        for channel in 0..3 {
            let background = f32::from(OCEAN_BACKGROUND[channel]);
            let delta = f32::from(color[channel]) - background;
            color[channel] = (background + weight * delta).round() as u8;
        }
    }
    color
}

// BitCraft's cube rounding followed by the large-hex odd-row offset.
fn offset_cell(x: f64, r: f64) -> Coord {
    let q = x / HEX_X - r / 2.;
    let s = -q - r;
    let (mut qi, mut ri, si) = (q.round(), r.round(), s.round());
    let (dq, dr, ds) = ((q - qi).abs(), (r - ri).abs(), (s - si).abs());
    if dq > ds && dq > dr {
        qi = -si - ri;
    } else if dr > ds {
        ri = -qi - si;
    }
    (qi as i32 + ri as i32 / 2, ri as i32)
}

fn raster_tile(
    (x, y): TileCoord,
    extent: [f64; 4],
    resolution: f64,
    colors: &ColorWorld,
) -> Option<RgbaImage> {
    let left = extent[0] + f64::from(x * TILE) * resolution;
    let top = extent[3] - f64::from(y * TILE) * resolution;
    // Keep pixels outside coverage the same color as the map backdrop.
    // Opaque ocean fill also avoids dark fringes when downsampling parent tiles.
    let mut image = RgbaImage::from_pixel(TILE, TILE, OCEAN_BACKGROUND);
    let mut has_terrain = false;
    for (px, py, pixel) in image.enumerate_pixels_mut() {
        let row = (top - (f64::from(py) + 0.5) * resolution) / HEX_Z;
        let (ox, oz) = offset_cell(left + (f64::from(px) + 0.5) * resolution, row);
        if let Some(color) = colors.sample(ox, oz) {
            *pixel = color;
            has_terrain = true;
        }
    }
    has_terrain.then_some(image)
}

fn tile_path(root: &Path, z: u8, (x, y): TileCoord) -> PathBuf {
    root.join(z.to_string())
        .join(x.to_string())
        .join(format!("{y}.webp"))
}

fn save(root: &Path, z: u8, coord: TileCoord, image: &RgbaImage) -> Result<()> {
    let path = tile_path(root, z, coord);
    std::fs::create_dir_all(path.parent().unwrap())?;
    image
        .save(&path)
        .with_context(|| format!("Could not write {}", path.display()))
}

fn encode_delivery_tile(path: &Path, quality: u8) -> Result<()> {
    let image = image::open(path)?.into_rgb8();
    let encoded = webp::Encoder::from_rgb(image.as_raw(), image.width(), image.height())
        .encode_simple(false, f32::from(quality))
        .map_err(|err| anyhow::anyhow!("WebP encoder failed: {err:?}"))?;
    std::fs::write(path, &*encoded)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region() -> WorldRegionState {
        WorldRegionState {
            id: 0,
            region_index: 7,
            region_count: 25,
            region_count_sqrt: 5,
            region_min_chunk_x: 2,
            region_min_chunk_z: 2,
            region_width_chunks: 2,
            region_height_chunks: 2,
        }
    }

    #[test]
    fn layout_preserves_region_numbers_and_inactive_bounds() {
        let layout = Layout::from_region(&region()).unwrap();
        assert_eq!(layout.origin, (0, 0));
        let top_right = layout.region(25, None);
        assert_eq!((top_right.x, top_right.z, top_right.active), (8, 8, false));
    }

    #[test]
    fn hex_centers_and_staggered_rows_round_trip() {
        for z in 0..32 {
            for x in 0..32 {
                let world_x = (f64::from(x) + f64::from(z % 2) / 2.) * HEX_X;
                assert_eq!(offset_cell(world_x, f64::from(z)), (x, z));
            }
        }
    }

    #[test]
    fn shades_land_across_every_chunk_edge_including_the_first_east_cell() {
        let mut world = World::default();
        for (coord, elevation) in [
            ((1, 1), 0),
            ((0, 1), 2),
            ((2, 1), 3),
            ((1, 0), 5),
            ((1, 2), 7),
        ] {
            world.chunks.insert(
                coord,
                Chunk {
                    biomes: vec![1; CELLS],
                    elevations: vec![elevation; CELLS],
                    water: vec![0; CELLS],
                },
            );
        }
        let colors = ColorWorld::from_world(
            world,
            Layout {
                origin: (0, 0),
                region_size: (3, 3),
                side: 1,
            },
        );
        for (x, z, shade) in [
            (0, 1, -2.),
            (CHUNK - 1, 1, 3.),
            (1, 0, 5.),
            (1, CHUNK - 1, -7.),
            (CHUNK - 1, 0, 8.),
        ] {
            assert_eq!(
                colors.sample(CHUNK + x, CHUNK + z),
                Some(tint(BIOMES[1], [0.; 3], shade * LAND_SLOPE_STRENGTH)),
                "incorrect shading at local terrain cell ({x}, {z})"
            );
        }
    }

    #[test]
    fn shallow_water_stays_slate_blue_and_deep_water_matches_the_backdrop() {
        let shallows = water_color(1);
        assert!(shallows[0] < shallows[1] && shallows[1] < shallows[2]);
        assert!(shallows[2] > OCEAN_BACKGROUND[2] && shallows[2] - OCEAN_BACKGROUND[2] <= 8);
        assert_eq!(shallows[1] - shallows[0], OCEAN[1] - OCEAN[0]);
        assert_eq!(water_color(100), OCEAN_BACKGROUND);
    }

    #[test]
    fn feathers_water_at_missing_terrain_but_not_between_present_chunks() {
        let mut world = World::default();
        for z in 0..3 {
            for x in 0..3 {
                let mut water = vec![1; CELLS];
                if (x, z) == (0, 1) {
                    water[CHUNK as usize] = 0;
                }
                world.chunks.insert(
                    (x, z),
                    Chunk {
                        biomes: vec![1; CELLS],
                        elevations: vec![0; CELLS],
                        water,
                    },
                );
            }
        }
        let colors = ColorWorld::from_world(
            world,
            Layout {
                origin: (0, 0),
                region_size: (3, 3),
                side: 1,
            },
        );
        let z = CHUNK + CHUNK / 2;
        assert_eq!(colors.sample(0, z), Some(OCEAN_BACKGROUND));
        let middle = colors.sample(12, z).unwrap();
        let unblended = colors.sample(24, z).unwrap();
        assert!(middle[1] > OCEAN_BACKGROUND[1] && middle[1] < unblended[1]);
        assert_eq!(colors.sample(CHUNK - 1, z), Some(unblended));
        assert_eq!(colors.sample(CHUNK, z), Some(unblended));
        assert_eq!(
            colors.sample(0, CHUNK + 1),
            Some(tint(BIOMES[1], [0.; 3], 0.))
        );
    }

    #[test]
    fn feathers_a_missing_diagonal_without_fading_intact_edges() {
        let water = Rgba([45, 90, 110, 255]);
        let edges = [false; 4];
        let diagonals = [true, false, false, false];
        assert_eq!(
            feather_water(water, edges, diagonals, 0, 0),
            OCEAN_BACKGROUND
        );
        assert_eq!(feather_water(water, edges, diagonals, 0, 31), water);
        assert_eq!(feather_water(water, edges, diagonals, 31, 0), water);
    }

    #[test]
    fn builds_a_sparse_pyramid_from_a_terrain_chunk() {
        let root =
            std::env::temp_dir().join(format!("bitcraft-map-test-{:016x}", rand::random::<u64>()));
        let mut world = World::default();
        world
            .insert(
                region(),
                Some("Test region".into()),
                vec![TerrainChunkState {
                    chunk_index: 2003,
                    chunk_x: 2,
                    chunk_z: 2,
                    dimension: 1,
                    biomes: vec![1; CELLS],
                    elevations: vec![10; CELLS],
                    water_levels: vec![0; CELLS],
                    biome_density: vec![],
                    water_body_types: vec![],
                    zoning_types: vec![],
                    original_elevations: vec![],
                }],
            )
            .unwrap();
        let manifest = world.build(&root, "test".into(), 78).unwrap();
        assert_eq!(manifest.regions[6].chunks, 1);
        assert!(manifest.regions[6].active);
        assert_eq!(manifest.regions[6].name.as_deref(), Some("Test region"));
        assert!(manifest.regions[0].name.is_none());
        assert_eq!(manifest.background_color, "#282f41");
        assert_eq!(manifest.tiles[0], vec![0]);
        assert!(manifest.tiles.last().unwrap().len() < 4);
        let tile = std::fs::read(tile_path(&root, 0, (0, 0))).unwrap();
        assert_eq!(&tile[12..16], b"VP8 ");
        let image = image::load_from_memory(&tile).unwrap().into_rgba8();
        assert!(image.pixels().all(|p| p[3] == 255));
        let distance = |pixel: &Rgba<u8>| {
            (0..3)
                .map(|i| pixel[i].abs_diff(OCEAN_BACKGROUND[i]))
                .max()
                .unwrap()
        };
        assert!(distance(image.get_pixel(0, 0)) <= 4);
        assert!(image.pixels().any(|p| distance(p) > 16));
        std::fs::remove_dir_all(root).unwrap();
    }
}
