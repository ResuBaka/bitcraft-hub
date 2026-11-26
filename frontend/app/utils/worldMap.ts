type WorldExtent = [west: number, south: number, east: number, north: number];

// BitCraftPublic: terrain hex radius 10, row spacing 1.5 radii,
// 32 x 32 terrain cells per chunk. OpenLayers y is BitCraft z (north).
export const CHUNK_WIDTH = 32 * 10 * Math.sqrt(3);
export const CHUNK_HEIGHT = 32 * 15;
export const SMALL_HEX_WIDTH = (10 * Math.sqrt(3)) / 3;
export const SMALL_HEX_ROW_HEIGHT = 5;

export interface WorldMapRegion {
  id: number;
  x: number;
  z: number;
  width: number;
  height: number;
  chunks: number;
  active?: boolean;
  name?: string;
}

export interface WorldMapClaim {
  id: string;
  name: string;
  tier: number;
  region: number;
  x: number;
  z: number;
  tiles: number;
  members: number;
}

export function compareClaimMarkerPriority(
  a: Pick<WorldMapClaim, "tier" | "id">,
  b: Pick<WorldMapClaim, "tier" | "id">,
): number {
  // OpenLayers keeps earlier features when badges overlap. Break ties stably
  // without converting entity IDs to JavaScript numbers.
  return b.tier - a.tier || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0);
}

const CLAIM_BADGE_SIZES = [
  { radius: 6, fontSize: 8 },
  { radius: 7, fontSize: 9 },
  { radius: 9, fontSize: 11 },
  { radius: 11, fontSize: 12 },
] as const;

export function claimBadgeSize(resolution: number) {
  // Resolution is world units per CSS pixel. Keep badges centered on their
  // flag, using a smaller collision footprint at wider zooms.
  return resolution >= 128
    ? CLAIM_BADGE_SIZES[0]
    : resolution >= 48
      ? CLAIM_BADGE_SIZES[1]
      : resolution >= 16
        ? CLAIM_BADGE_SIZES[2]
        : CLAIM_BADGE_SIZES[3];
}

// Claim flags are small-hex offset coordinates. A small hex has one third of
// the terrain hex radius; odd rows shift east by half a cell.
export function smallHexCenter(x: number, z: number): [number, number] {
  const shift = z / 2 - Math.trunc(z / 2);
  return [(x + shift) * SMALL_HEX_WIDTH, z * SMALL_HEX_ROW_HEIGHT];
}

const SMALL_HEX_CORNERS = [
  [1, -1],
  [1, 1],
  [0, 2],
  [-1, 1],
  [-1, -1],
  [0, -2],
] as const;

// Edges are ordered E, NE, NW, W, SW, SE. Project their endpoints from
// integer lattice coordinates so shared corners match even far from the origin.
export function smallHexEdgeSegments([x, z, mask]: readonly [
  number,
  number,
  number,
]): number[][][] {
  const segments: number[][][] = [];
  const vertex = ([dx, dz]: (typeof SMALL_HEX_CORNERS)[number]) => [
    (2 * x + (z & 1) + dx) * (SMALL_HEX_WIDTH / 2),
    ((3 * z + dz) * SMALL_HEX_ROW_HEIGHT) / 3,
  ];
  for (const [edge, a] of SMALL_HEX_CORNERS.entries()) {
    if (mask & (1 << edge)) segments.push([vertex(a), vertex(SMALL_HEX_CORNERS[(edge + 1) % 6]!)]);
  }
  return segments;
}

export interface WorldMapLayout {
  label: string;
  dimension: number;
  backgroundColor?: string;
  regions: WorldMapRegion[];
}

export interface WorldMapAtlas extends WorldMapLayout {
  version: string;
  extent: WorldExtent;
  tileSize: number;
  resolutions: number[];
  tiles: number[][];
}

const MAIN_REGION_IDS = new Set([7, 8, 9, 12, 13, 14, 17, 18, 19]);

export function isAtlasEventRegion(id: number) {
  return !MAIN_REGION_IDS.has(id);
}

export const CURRENT_WORLD_LAYOUT: WorldMapLayout = {
  label: "Current region layout",
  dimension: 1,
  regions: Array.from({ length: 25 }, (_, index) => {
    const x = index % 5;
    const z = Math.floor(index / 5);
    return {
      id: index + 1,
      x,
      z,
      width: 1,
      height: 1,
      chunks: 0,
      active: !isAtlasEventRegion(index + 1),
    };
  }),
};

// The current layout confirms positions in the grid, not live world coordinates.
export function schematicRegionExtent(region: WorldMapRegion): WorldExtent {
  return [region.x, region.z, region.x + region.width, region.z + region.height];
}

export function regionExtent(region: WorldMapRegion): WorldExtent {
  // Rectangular boundaries split the centers of adjacent rows and average
  // the half-cell X offset of odd rows. The raster keeps the true hex edges.
  const west = (region.x - 1 / 4 / 32) * CHUNK_WIDTH;
  const south = (region.z - 1 / 2 / 32) * CHUNK_HEIGHT;
  return [west, south, west + region.width * CHUNK_WIDTH, south + region.height * CHUNK_HEIGHT];
}

export function regionCoverage(region: WorldMapRegion) {
  if (!region.chunks) return "No terrain";
  if (region.chunks < region.width * region.height) return "Partial terrain";
  return "Terrain available";
}

export function atlasTileKey(zoom: number, x: number, y: number) {
  const side = 2 ** zoom;
  return x < 0 || y < 0 || x >= side || y >= side ? -1 : y * side + x;
}
