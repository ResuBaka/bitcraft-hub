import { describe, expect, test } from "bun:test";
import {
  atlasTileKey,
  claimBadgeSize,
  compareClaimMarkerPriority,
  CHUNK_HEIGHT,
  CHUNK_WIDTH,
  CURRENT_WORLD_LAYOUT,
  regionCoverage,
  regionExtent,
  schematicRegionExtent,
  smallHexCenter,
} from "../app/utils/worldMap";

describe("world atlas", () => {
  test("prioritizes higher claim tiers with a stable tie break", () => {
    const lower = { id: "504403158266578088", tier: 3 };
    const highA = { id: "504403158266578089", tier: 8 };
    const highB = { id: "504403158266578090", tier: 8 };
    expect([lower, highB, highA].sort(compareClaimMarkerPriority)).toEqual([highA, highB, lower]);
  });

  test("keeps claim badges smaller at wider zooms", () => {
    const sizes = [160, 64, 24, 4].map(claimBadgeSize);
    expect(sizes.map((size) => size.radius)).toEqual([6, 7, 9, 11]);
    expect(sizes.map((size) => size.fontSize)).toEqual([8, 9, 11, 12]);
  });

  test("current grid has 25 numbered positions and exactly the nine active regions", () => {
    const { regions } = CURRENT_WORLD_LAYOUT;
    expect(regions.map((region) => region.id)).toEqual(Array.from({ length: 25 }, (_, i) => i + 1));
    expect(regions.filter((region) => region.active).map((region) => region.id)).toEqual([
      7, 8, 9, 12, 13, 14, 17, 18, 19,
    ]);
    expect(schematicRegionExtent(regions[0]!)).toEqual([0, 0, 1, 1]);
    expect(schematicRegionExtent(regions[24]!)).toEqual([4, 4, 5, 5]);
  });

  test("uses a continuous X/Z chunk grid with +Z north", () => {
    const [west, south, east, north] = regionExtent({
      id: 1,
      x: 0,
      z: 0,
      width: 80,
      height: 80,
      chunks: 0,
    });
    const nextEast = regionExtent({ id: 2, x: 80, z: 0, width: 80, height: 80, chunks: 0 });
    const nextNorth = regionExtent({ id: 4, x: 0, z: 80, width: 80, height: 80, chunks: 0 });
    expect(east - west).toBeCloseTo(80 * CHUNK_WIDTH);
    expect(north - south).toBeCloseTo(80 * CHUNK_HEIGHT);
    expect(nextEast[0]).toBeCloseTo(east);
    expect(nextNorth[1]).toBeCloseTo(north);
  });

  test("projects claim flags from the staggered small-hex grid onto terrain", () => {
    const cellWidth = CHUNK_WIDTH / 96;
    expect(smallHexCenter(3, 0)).toEqual([3 * cellWidth, 0]);
    const [x, z] = smallHexCenter(3, 3);
    expect(x).toBeCloseTo(3.5 * cellWidth);
    expect(z).toBe(CHUNK_HEIGHT / 32);
    expect(smallHexCenter(0, -1)[0]).toBeCloseTo(-cellWidth / 2);
  });

  test("distinguishes missing and partial coverage", () => {
    const region = { id: 1, x: 0, z: 0, width: 2, height: 2, chunks: 0 };
    expect(regionCoverage(region)).toBe("No terrain");
    expect(regionCoverage({ ...region, chunks: 2 })).toBe("Partial terrain");
    expect(regionCoverage({ ...region, chunks: 4 })).toBe("Terrain available");
  });

  test("tile keys reject out-of-range coordinates without aliasing neighbors", () => {
    expect(atlasTileKey(2, 0, 1)).toBe(4);
    expect(atlasTileKey(2, 4, 0)).toBe(-1);
    expect(atlasTileKey(2, -1, 1)).toBe(-1);
  });
});
