import type { ClaimDetailBounds } from "./claimMapDetails";
import type { WorldMapRegion } from "./worldMap";
import { SMALL_HEX_ROW_HEIGHT, SMALL_HEX_WIDTH } from "./worldMap";

export const MAX_RESOURCE_FILTERS = 8;
export const RESOURCE_MAX_RESOLUTION = 4;
export const RESOURCE_CLUSTER_BASE = 48;
const MAX_CLUSTER_LEVEL = 8;
const MAX_CLUSTER_CELLS = 4096;
const MAX_WORLD_CELL = 96_000;
const SMALL_CELLS_PER_CHUNK = 96;

export function atlasResourceIconSize(resolution: number) {
  // Each zoom step halves resolution. Grow by six pixels per step, with
  // half-pixel increments during animations and a cap at close zoom.
  const pixels = 12 + 6 * Math.log2(RESOURCE_MAX_RESOLUTION / resolution);
  return Math.max(12, Math.min(26, Math.round(pixels * 2) / 2));
}

export interface AtlasResourceType {
  id: number;
  name: string;
  tag: string;
  tier: number;
  icon_asset_name?: string;
}

export function atlasResourceIconUrl(asset: string | undefined, iconDomain: string) {
  const root = iconDomain.trim().replace(/\/+$/, "");
  if (!root) return;
  const name = asset
    ?.trim()
    .replace(/\[([^\]]+)\]$/, (_, choices: string) => (choices.split(",")[0] ?? "").trim());
  if (!name) return;
  const parts = name.split("/");
  if (parts.some((part) => !part || part === "." || part === ".." || part.includes("\\"))) return;
  return `${root}/${parts.map(encodeURIComponent).join("/")}.webp`;
}

export interface AtlasResourcesResponse {
  nodes: [x: number, z: number, id: number][];
  truncated: boolean;
}

export interface AtlasResourceClustersResponse {
  clusters: [
    bucketX: number,
    bucketZ: number,
    count: number,
    dominantResourceId: number,
    centerX2?: number,
    centerZ?: number,
  ][];
  partial: boolean;
}

export function clusterCellSize(level: number) {
  return RESOURCE_CLUSTER_BASE * 2 ** level;
}

export function resourceClusterBounds(bounds: ClaimDetailBounds, level: number): ClaimDetailBounds {
  const size = clusterCellSize(level);
  return {
    minX: Math.floor(bounds.minX / size),
    maxX: Math.floor(bounds.maxX / size),
    minZ: Math.floor(bounds.minZ / size),
    maxZ: Math.floor(bounds.maxZ / size),
  };
}

export function resourceClusterBoundsFit(bounds: ClaimDetailBounds) {
  return (bounds.maxX - bounds.minX + 1) * (bounds.maxZ - bounds.minZ + 1) <= MAX_CLUSTER_CELLS;
}

export function resourceClusterLevel(bounds: ClaimDetailBounds, resolution: number) {
  // Keep buckets near a fixed screen size as the camera zooms.
  const target = (56 * resolution) / SMALL_HEX_ROW_HEIGHT;
  let level = Math.min(
    MAX_CLUSTER_LEVEL,
    Math.max(0, Math.round(Math.log2(target / RESOURCE_CLUSTER_BASE))),
  );
  while (
    level < MAX_CLUSTER_LEVEL &&
    !resourceClusterBoundsFit(resourceClusterBounds(bounds, level))
  )
    level++;
  return level;
}

export function resourceClusterExtent(
  x: number,
  z: number,
  level: number,
): [number, number, number, number] {
  const size = clusterCellSize(level);
  return [
    x * size * SMALL_HEX_WIDTH,
    z * size * SMALL_HEX_ROW_HEIGHT,
    (x + 1) * size * SMALL_HEX_WIDTH,
    (z + 1) * size * SMALL_HEX_ROW_HEIGHT,
  ];
}

// The database buckets bound the query, but don't dictate badge placement.
// Merge nearby centers of the same dominant type, keeping the densest bucket
// as a fixed search anchor so a chain of neighbors can't swallow a region.
export function placeResourceClusters(
  clusters: AtlasResourceClustersResponse["clusters"],
  level: number,
) {
  type Badge = {
    id: number;
    count: number;
    center: [number, number];
    extent: ReturnType<typeof resourceClusterExtent>;
  };
  type Seed = { anchor: [number, number]; badge: Badge };
  const radius = clusterCellSize(level) * SMALL_HEX_ROW_HEIGHT * 0.8;
  const seeds = new Map<string, Seed>();
  const badges: Badge[] = [];
  const ordered = [...clusters].sort(
    (a, b) => b[2] - a[2] || a[0] - b[0] || a[1] - b[1] || a[3] - b[3],
  );
  // A cell's diagonal is the merge radius, so it holds at most one seed per type.
  const searchCell = radius / Math.SQRT2;
  for (const [x, z, count, id, centerX2, centerZ] of ordered) {
    const extent = resourceClusterExtent(x, z, level);
    const px =
      centerX2 === undefined ? (extent[0] + extent[2]) / 2 : (centerX2 * SMALL_HEX_WIDTH) / 2;
    const pz = centerZ === undefined ? (extent[1] + extent[3]) / 2 : centerZ * SMALL_HEX_ROW_HEIGHT;
    const ix = Math.floor(px / searchCell);
    const iz = Math.floor(pz / searchCell);
    let nearest: Seed | undefined;
    let distance = radius * radius;
    for (let dx = -2; dx <= 2; dx++) {
      for (let dz = -2; dz <= 2; dz++) {
        const seed = seeds.get(`${id}:${ix + dx}:${iz + dz}`);
        if (!seed) continue;
        const d = (px - seed.anchor[0]) ** 2 + (pz - seed.anchor[1]) ** 2;
        if (d < distance) {
          distance = d;
          nearest = seed;
        }
      }
    }
    if (!nearest) {
      const badge: Badge = { id, count, center: [px, pz], extent };
      seeds.set(`${id}:${ix}:${iz}`, { anchor: [px, pz], badge });
      badges.push(badge);
      continue;
    }
    const badge = nearest.badge;
    const weight = count / (badge.count + count);
    badge.center[0] += (px - badge.center[0]) * weight;
    badge.center[1] += (pz - badge.center[1]) * weight;
    badge.count += count;
    badge.extent[0] = Math.min(badge.extent[0], extent[0]);
    badge.extent[1] = Math.min(badge.extent[1], extent[1]);
    badge.extent[2] = Math.max(badge.extent[2], extent[2]);
    badge.extent[3] = Math.max(badge.extent[3], extent[3]);
  }
  return badges;
}

export function resourceClusterWorldBounds(
  bounds: ClaimDetailBounds,
  level: number,
): ClaimDetailBounds {
  const size = clusterCellSize(level);
  return {
    minX: bounds.minX * size,
    maxX: Math.min(MAX_WORLD_CELL, (bounds.maxX + 1) * size - 1),
    minZ: bounds.minZ * size,
    maxZ: Math.min(MAX_WORLD_CELL, (bounds.maxZ + 1) * size - 1),
  };
}

export function atlasResourceIds(ids: readonly number[]): number[] {
  return [
    ...new Set(
      ids.filter((id) => Number.isInteger(id) && id >= -2_147_483_648 && id <= 2_147_483_647),
    ),
  ]
    .slice(0, MAX_RESOURCE_FILTERS)
    .sort((a, b) => a - b);
}

export function atlasResourceRegions(
  bounds: ClaimDetailBounds,
  regions: readonly WorldMapRegion[],
) {
  return regions
    .filter(
      (region) =>
        (region.active ?? region.chunks > 0) &&
        bounds.maxX >= region.x * SMALL_CELLS_PER_CHUNK &&
        bounds.minX < (region.x + region.width) * SMALL_CELLS_PER_CHUNK &&
        bounds.maxZ >= region.z * SMALL_CELLS_PER_CHUNK &&
        bounds.minZ < (region.z + region.height) * SMALL_CELLS_PER_CHUNK,
    )
    .map(({ id }) => id)
    .sort((a, b) => a - b);
}

export type AtlasResourceColors = Readonly<Record<number, string>>;

export function normalizeAtlasResourceColor(value: unknown): string | undefined {
  if (typeof value !== "string") return;
  let hex = value.trim().replace(/^#/, "").toLowerCase();
  if (/^[\da-f]{3}$/.test(hex)) hex = [...hex].map((c) => c.repeat(2)).join("");
  if (/^[\da-f]{6}$/.test(hex)) return `#${hex}`;
}

export function atlasResourceColor(id: number, custom?: AtlasResourceColors): string {
  if (custom?.[id]) return custom[id];
  const hash = Math.imul(id, 0x9e3779b9) >>> 0;
  const hue = (hash / 0x1_0000_0000) * 6;
  const chroma = 0.51; // HSL: 85% saturation, 70% lightness
  const second = chroma * (1 - Math.abs((hue % 2) - 1));
  const [r, g, b] = [
    [chroma, second, 0],
    [second, chroma, 0],
    [0, chroma, second],
    [0, second, chroma],
    [second, 0, chroma],
    [chroma, 0, second],
  ][Math.floor(hue)]!;
  const hex = (value: number) =>
    Math.round((value + 0.445) * 255)
      .toString(16)
      .padStart(2, "0");
  return `#${hex(r!)}${hex(g!)}${hex(b!)}`;
}

export function atlasResourceTextColor(hex: string) {
  const linear = (offset: number) => {
    const channel = Number.parseInt(hex.slice(offset, offset + 2), 16) / 255;
    return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
  };
  const luminance = 0.2126 * linear(1) + 0.7152 * linear(3) + 0.0722 * linear(5);
  return luminance > 0.21 ? "#17212b" : "#ffffff";
}

export function matchAtlasResource(resource: AtlasResourceType, search: string) {
  const terms = search.toLowerCase().split(/\s+/).filter(Boolean);
  const entry =
    `${resource.name} ${resource.tag} t${resource.tier} tier ${resource.tier}`.toLowerCase();
  return terms.every((term) => entry.includes(term));
}
