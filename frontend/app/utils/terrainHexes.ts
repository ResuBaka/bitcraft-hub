import { atlasTileKey, CHUNK_HEIGHT, CHUNK_WIDTH, type WorldMapAtlas } from "./worldMap";

const HEX_WIDTH = CHUNK_WIDTH / 32;
const HEX_ROW_HEIGHT = CHUNK_HEIGHT / 32;
const HEX_RADIUS = 10;
const MAX_VISIBLE_CELLS = 8_192;

export const TERRAIN_HEX_MAX_RESOLUTION = 0.58;
export const TERRAIN_MIN_RESOLUTION = 0.05;
export const TERRAIN_HEX_MAX_SOURCE_RESOLUTION = HEX_RADIUS / 2;

export interface TerrainHexBounds {
  minX: number;
  minZ: number;
  maxX: number;
  maxZ: number;
}

export interface TerrainRasterSample {
  key: number;
  x: number;
  y: number;
  zoom: number;
  pixel: number;
}

export function terrainHexCenter(x: number, z: number): [number, number] {
  const shift = z / 2 - Math.trunc(z / 2);
  return [(x + shift) * HEX_WIDTH, z * HEX_ROW_HEIGHT];
}

export function terrainHexRing(cx: number, cz: number): number[][] {
  const inner = HEX_WIDTH / 2;
  const half = HEX_RADIUS / 2;
  return [
    [cx + inner, cz - half],
    [cx + inner, cz + half],
    [cx, cz + HEX_RADIUS],
    [cx - inner, cz + half],
    [cx - inner, cz - half],
    [cx, cz - HEX_RADIUS],
    [cx + inner, cz - half],
  ];
}

export function terrainHexBounds(extent: number[]): TerrainHexBounds | undefined {
  const [west, south, east, north] = extent;
  if (
    west === undefined ||
    south === undefined ||
    east === undefined ||
    north === undefined ||
    ![west, south, east, north].every(Number.isFinite)
  )
    return;
  // Include the neighbors whose corners may reach into the viewport.
  const bounds = {
    minX: Math.floor(west / HEX_WIDTH) - 1,
    minZ: Math.floor(south / HEX_ROW_HEIGHT) - 1,
    maxX: Math.ceil(east / HEX_WIDTH) + 1,
    maxZ: Math.ceil(north / HEX_ROW_HEIGHT) + 1,
  };
  const count = (bounds.maxX - bounds.minX + 1) * (bounds.maxZ - bounds.minZ + 1);
  return count > 0 && count <= MAX_VISIBLE_CELLS ? bounds : undefined;
}

export function containsTerrainHexBounds(outer: TerrainHexBounds, inner: TerrainHexBounds) {
  return (
    outer.minX <= inner.minX &&
    outer.minZ <= inner.minZ &&
    outer.maxX >= inner.maxX &&
    outer.maxZ >= inner.maxZ
  );
}

export function terrainPixelSampler(
  data: WorldMapAtlas,
): ((x: number, z: number) => TerrainRasterSample | undefined) | undefined {
  const zoom = data.resolutions.length - 1;
  const resolution = data.resolutions[zoom];
  const { tileSize } = data;
  if (resolution === undefined || resolution <= 0 || resolution > TERRAIN_HEX_MAX_SOURCE_RESOLUTION)
    return;
  const [west, , , north] = data.extent;
  const keys = new Set(data.tiles[zoom]);
  return (x: number, z: number) => {
    // The raster has a north-west origin and Y increases southwards.
    const imageX = Math.floor((x - west) / resolution);
    const imageY = Math.floor((north - z) / resolution);
    const tileX = Math.floor(imageX / tileSize);
    const tileY = Math.floor(imageY / tileSize);
    const key = atlasTileKey(zoom, tileX, tileY);
    if (!keys.has(key)) return;
    const pixel = ((imageY % tileSize) * tileSize + (imageX % tileSize)) * 4;
    return { key, x: tileX, y: tileY, zoom, pixel };
  };
}
