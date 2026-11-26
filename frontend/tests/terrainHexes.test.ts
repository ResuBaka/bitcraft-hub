import { describe, expect, test } from "bun:test";
import {
  containsTerrainHexBounds,
  terrainHexBounds,
  terrainHexCenter,
  terrainHexRing,
  terrainPixelSampler,
} from "../app/utils/terrainHexes";
import { CHUNK_HEIGHT, CHUNK_WIDTH, type WorldMapAtlas } from "../app/utils/worldMap";

const atlas: WorldMapAtlas = {
  version: "fixture",
  label: "Fixture terrain",
  dimension: 1,
  extent: [0, 0, 4096, 4096],
  tileSize: 512,
  resolutions: [8, 4],
  tiles: [[0], [0, 1]],
  regions: [],
};

describe("close zoom terrain", () => {
  test("aligns crisp terrain hexes to the staggered rows of the raster", () => {
    const width = CHUNK_WIDTH / 32;
    const [x, z] = terrainHexCenter(1, 1);
    expect(x).toBeCloseTo(width * 1.5);
    expect(z).toBe(CHUNK_HEIGHT / 32);
    const ring = terrainHexRing(0, 0);
    const next = terrainHexRing(...terrainHexCenter(1, 0));
    expect(ring[0]).toEqual(next[4]);
    expect(ring[1]).toEqual(next[3]);
    expect(ring[0]).toEqual(ring.at(-1));
  });

  test("reads the correct pixel across tile boundaries with north at the top", () => {
    const sample = terrainPixelSampler(atlas)!;
    expect(sample(2047, 4095)).toEqual({ key: 0, x: 0, y: 0, zoom: 1, pixel: 511 * 4 });
    expect(sample(2048, 4091)).toEqual({ key: 1, x: 1, y: 0, zoom: 1, pixel: 512 * 4 });
    expect(sample(2048, 2000)).toBeUndefined();
    expect(terrainPixelSampler({ ...atlas, resolutions: [8] })).toBeUndefined();
  });

  test("bounds the number of polygons rendered and reuses buffered areas", () => {
    const visible = terrainHexBounds([0, 0, 100, 100])!;
    const buffered = terrainHexBounds([-25, -25, 125, 125])!;
    expect(containsTerrainHexBounds(buffered, visible)).toBe(true);
    expect(terrainHexBounds([0, 0, 30_000, 30_000])).toBeUndefined();
  });
});
