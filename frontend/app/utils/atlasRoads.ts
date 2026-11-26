import { claimBuildingBounds } from "./claimMapDetails";
import {
  SMALL_HEX_ROW_HEIGHT,
  SMALL_HEX_WIDTH,
  smallHexCenter,
  smallHexEdgeSegments,
} from "./worldMap";

export const ROAD_MAX_RESOLUTION = 12;
const MAX_SPAN = 8192;

// Older API versions omit the type; those tiles keep their original color.
export type RoadRun = [firstX: number, lastX: number, z: number, tileTypeId?: number];
export interface AtlasRoadsResponse {
  runs: RoadRun[];
  tiers?: Record<number, number>;
  truncated: boolean;
  visibleOnly?: boolean;
  // A value above one means the runs are in overview bucket coordinates.
  scale?: number;
}

const ROAD_TIER_COLORS = [
  "#868D98",
  "#C68B50", // Clay-orange paving
  "#929C60", // Olive paving
  "#788FA3", // Blue-gray paving
  "#AB7BB2",
  "#CC6672",
  "#D6AC43",
  "#78BABA",
  "#4D505B",
  "#A9BBC6",
] as const;

export function roadTileColor(tileTypeId: number | undefined, tier?: number): string {
  const tierColor = tier === undefined ? undefined : ROAD_TIER_COLORS[tier - 1];
  if (tierColor) return tierColor;
  if (tileTypeId === undefined) return "#d2b79a";
  // Golden-ratio hashing spreads consecutive IDs around the hue wheel.
  // Unknown tiers fall back to a stable tile-type color.
  const hash = Math.imul(tileTypeId, 0x9e3779b9) >>> 0;
  const hue = (hash / 0x1_0000_0000) * 360;
  const lightness = [64, 70, 76][hash % 3];
  return `hsl(${hue.toFixed(2)}, 55%, ${lightness}%)`;
}

export function atlasRoadBounds(extent: number[], buffered = false) {
  const bounds = claimBuildingBounds(extent, buffered);
  if (bounds && bounds.maxX - bounds.minX < MAX_SPAN && bounds.maxZ - bounds.minZ < MAX_SPAN) {
    return bounds;
  }
}

export function roadRunRing([firstX, lastX, z]: RoadRun): number[][] {
  const [cx, cz] = smallHexCenter(firstX, z);
  const radius = 10 / 3;
  const shoulder = radius / 2;
  const inner = SMALL_HEX_WIDTH / 2;
  const left = [cx - inner, cz - shoulder];
  const ring = [left, [cx - inner, cz + shoulder]];
  const count = lastX - firstX + 1;
  // Adjacent hexes share a vertical edge. Walk the outside of the run
  // instead of generating overlapping polygons and seams for every tile.
  for (let i = 0; i < count; i++) {
    const x = cx + i * SMALL_HEX_WIDTH;
    ring.push([x, cz + radius], [x + inner, cz + shoulder]);
  }
  ring.push([cx + (count - 1) * SMALL_HEX_WIDTH + inner, cz - shoulder]);
  for (let i = count - 1; i >= 0; i--) {
    const x = cx + i * SMALL_HEX_WIDTH;
    ring.push([x, cz - radius], i === 0 ? left : [x - inner, cz - shoulder]);
  }
  return ring;
}

export function roadHexBorders(runs: readonly RoadRun[]): number[][][] {
  const rows = new Map<number, Set<number>>();
  for (const [firstX, lastX, z] of runs) {
    let row = rows.get(z);
    if (!row) rows.set(z, (row = new Set()));
    for (let x = firstX; x <= lastX; x++) row.add(x);
  }
  const segments: number[][][] = [];
  for (const [z, row] of rows) {
    const south = rows.get(z - 1);
    const odd = z & 1;
    for (const x of row) {
      // Always draw E, NE and NW. Only draw their counterparts on the
      // perimeter, so neighbors of any paving color share a single stroke.
      let mask = 0b000111;
      if (!row.has(x - 1)) mask |= 1 << 3;
      if (!south?.has(x + odd - 1)) mask |= 1 << 4;
      if (!south?.has(x + odd)) mask |= 1 << 5;
      segments.push(...smallHexEdgeSegments([x, z, mask]));
    }
  }
  return segments;
}

export function roadOverviewRunRing([firstX, lastX, z]: RoadRun, scale: number): number[][] {
  const west = (firstX * scale - 0.25) * SMALL_HEX_WIDTH;
  const east = ((lastX + 1) * scale - 0.25) * SMALL_HEX_WIDTH;
  const south = (z * scale - 0.5) * SMALL_HEX_ROW_HEIGHT;
  const north = ((z + 1) * scale - 0.5) * SMALL_HEX_ROW_HEIGHT;
  return [
    [west, south],
    [west, north],
    [east, north],
    [east, south],
    [west, south],
  ];
}
