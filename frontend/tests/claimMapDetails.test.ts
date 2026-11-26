import { describe, expect, test } from "bun:test";
import {
  claimBorderSegments,
  claimBuildingBounds,
  claimDetailBounds,
  containsClaimBounds,
} from "../app/utils/claimMapDetails";

describe("claim details overlay", () => {
  test("projects shared small-hex edges to the same endpoints", () => {
    const [[eastA, eastB]] = claimBorderSegments([0, 0, 1]) as [[number[], number[]]];
    const [[westA, westB]] = claimBorderSegments([1, 0, 1 << 3]) as [[number[], number[]]];
    expect(eastA).toEqual(westB);
    expect(eastB).toEqual(westA);
    expect(claimBorderSegments([0, 0, 0])).toEqual([]);
  });

  test("adds room for bordering hexes and can cache buffered bounds", () => {
    const extent = [300, 400, 1000, 1100];
    const visible = claimDetailBounds(extent)!;
    expect(visible.minX).toBeLessThan(300 / 5.77);
    expect(containsClaimBounds(claimDetailBounds(extent, true)!, visible)).toBe(true);
    expect(claimDetailBounds([0, 0, 100_000, 100_000])).toBeUndefined();
    expect(claimDetailBounds([-400, -400, -300, -300])).toBeUndefined();
  });

  test("building-only lookups accept regional views that are too large for borders", () => {
    const extent = [20_000, 30_000, 112_000, 95_000];
    expect(claimDetailBounds(extent)).toBeUndefined();
    const visible = claimBuildingBounds(extent)!;
    const buffered = claimBuildingBounds(extent, true)!;
    expect(containsClaimBounds(buffered, visible)).toBe(true);
    expect(claimBuildingBounds([-500, -500, -400, -400])).toBeUndefined();
    expect(claimBuildingBounds([-100, -100, 1_000_000, 1_000_000])).toEqual({
      minX: 0,
      minZ: 0,
      maxX: 96_000,
      maxZ: 96_000,
    });
  });
});
