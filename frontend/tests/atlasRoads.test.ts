import { expect, test } from "bun:test";
import {
  atlasRoadBounds,
  roadHexBorders,
  roadOverviewRunRing,
  roadRunRing,
  roadTileColor,
} from "../app/utils/atlasRoads";
import { SMALL_HEX_WIDTH, smallHexCenter, smallHexEdgeSegments } from "../app/utils/worldMap";

test("paved runs use staggered small hexes without internal vertical edges", () => {
  const [x, z] = smallHexCenter(1, 3);
  const ring = roadRunRing([1, 3, 3]);
  expect(ring[0]).toEqual(ring.at(-1));
  expect(Math.min(...ring.map(([x]) => x!))).toBeCloseTo(x - SMALL_HEX_WIDTH / 2);
  expect(Math.max(...ring.map(([x]) => x!))).toBeCloseTo(x + SMALL_HEX_WIDTH * 2.5);
  expect(Math.max(...ring.map(([, z]) => z!))).toBeCloseTo(z + 10 / 3);
  expect(ring).toHaveLength(15);
  expect(roadRunRing([1, 1, 3])).toHaveLength(7);
});

test("close paving outlines restore the vertical edges hidden inside each run", () => {
  const borders = roadHexBorders([[28_530, 28_532, 18_903]]);
  expect(borders).toHaveLength(16);
  for (const x of [28_530, 28_531]) {
    expect(borders).toContainEqual(smallHexEdgeSegments([x, 18_903, 1])[0]);
  }
});

test("paving tiles of different colors share exactly one diagonal edge on staggered rows", () => {
  for (const z of [18_903, 18_904]) {
    const x = 28_530;
    const borders = roadHexBorders([
      [x, x, z, 11],
      [x + (z & 1), x + (z & 1), z + 1, 12],
    ]);
    const [edge] = smallHexEdgeSegments([x, z, 1 << 1]);
    expect(borders).toHaveLength(11);
    expect(borders).toContainEqual(edge);
    expect(borders).not.toContainEqual([...edge!].reverse());
  }
});

test("road requests accept a wider area than close-up borders but remain bounded", () => {
  expect(atlasRoadBounds([0, 0, 25_000, 25_000], true)).toBeDefined();
  expect(atlasRoadBounds([0, 0, 100_000, 100_000])).toBeUndefined();
});

test("paving overview runs cover whole, adjoining buckets in world coordinates", () => {
  const west = roadOverviewRunRing([1, 3, 2], 4);
  const east = roadOverviewRunRing([4, 4, 2], 4);
  expect(west[0]).toEqual(west.at(-1));
  expect(west[2]![0]).toBe(east[0]![0]);
  expect(west[2]![0]! - west[0]![0]!).toBeCloseTo(12 * SMALL_HEX_WIDTH);
  expect(west[2]![1]! - west[0]![1]!).toBe(20);
});

test("paving colors are stable by tile type and legacy tiles stay neutral", () => {
  const ids = [1, 2, 3, 17, -91, -2_147_483_648, 2_147_483_647];
  const colors = ids.map((id) => roadTileColor(id));
  expect(colors.every((color) => /^hsl\(\d+\.\d+, 55%, \d+%\)$/.test(color))).toBe(true);
  expect(new Set(colors).size).toBe(ids.length);
  expect(
    ids
      .slice()
      .reverse()
      .map((id) => roadTileColor(id)),
  ).toEqual(colors.slice().reverse());
  expect(roadTileColor(undefined)).toBe("#d2b79a");
});

test("known roads use the in-game-inspired tier palette regardless of tile type", () => {
  const palette = [
    "#868D98",
    "#C68B50",
    "#929C60",
    "#788FA3",
    "#AB7BB2",
    "#CC6672",
    "#D6AC43",
    "#78BABA",
    "#4D505B",
    "#A9BBC6",
  ];
  expect(palette.map((_, i) => roadTileColor(-91, i + 1))).toEqual(palette);
  expect(roadTileColor(5, 3)).toBe(roadTileColor(12345, 3));
  expect(roadTileColor(3, 11)).toBe(roadTileColor(3));
});
