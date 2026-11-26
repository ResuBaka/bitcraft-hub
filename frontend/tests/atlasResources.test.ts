import { expect, test } from "bun:test";
import {
  atlasResourceIconSize,
  atlasResourceIconUrl,
  atlasResourceRegions,
  matchAtlasResource,
  placeResourceClusters,
  resourceClusterBounds,
  resourceClusterBoundsFit,
  resourceClusterExtent,
  resourceClusterLevel,
  resourceClusterWorldBounds,
} from "../app/utils/atlasResources";
import { SMALL_HEX_WIDTH, smallHexCenter } from "../app/utils/worldMap";

test("resource icons grow gradually as you zoom in and stay within their size limits", () => {
  expect([4, 2, 1.5, 1, 0.25].map(atlasResourceIconSize)).toEqual([12, 18, 20.5, 24, 26]);
  expect(atlasResourceIconSize(1.49)).toBe(atlasResourceIconSize(1.51));
});

test("resource icons use the in-game sprite path with a stable variant", () => {
  expect(atlasResourceIconUrl("GeneratedIcons/Other/Bush", "/sprites")).toBe(
    "/sprites/GeneratedIcons/Other/Bush.webp",
  );
  expect(atlasResourceIconUrl("Items/Ore[ 1, 2]", "https://icons.example/sprites/")).toBe(
    "https://icons.example/sprites/Items/Ore1.webp",
  );
  expect(atlasResourceIconUrl(undefined, "")).toBeUndefined();
  expect(atlasResourceIconUrl("GeneratedIcons/Other/Bush", "")).toBeUndefined();
  expect(atlasResourceIconUrl("../Bush", "/sprites")).toBeUndefined();
});

test("catalog search supports name, category, and tier", () => {
  const resource = { id: 7, name: "Blue Spruce", tag: "Tree", tier: 3 };
  expect(matchAtlasResource(resource, "blue tree t3")).toBe(true);
  expect(matchAtlasResource(resource, "blue tier 2")).toBe(false);
});

test("resource badges follow their nodes and merge close buckets across grid boundaries", () => {
  const badges = placeResourceClusters(
    [
      [0, 0, 30, 12, 95, 15],
      [1, 0, 10, 12, 97, 15],
      [2, 0, 5, 12, 192, 8],
      [0, 1, 8, 8, 94, 48],
    ],
    0,
  );
  expect(badges).toHaveLength(3);
  expect(badges[0]?.count).toBe(40);
  expect(badges[0]?.center[0]).toBeCloseTo(47.75 * SMALL_HEX_WIDTH);
  expect(badges[0]?.center[1]).toBe(75);
  expect(badges[0]?.extent).toEqual([0, 0, 96 * SMALL_HEX_WIDTH, 240]);
  expect(badges[1]).toMatchObject({ id: 8, count: 8 });
  expect(badges[2]?.center).toEqual(smallHexCenter(96, 8));
});

test("resource queries target only active regional modules intersecting the viewport", () => {
  const region = { z: 0, width: 100, height: 100, chunks: 50 };
  expect(
    atlasResourceRegions({ minX: 9500, maxX: 9800, minZ: 20, maxZ: 500 }, [
      { ...region, id: 8, x: 100 },
      { ...region, id: 7, x: 0 },
      { ...region, id: 9, x: 200 },
      { ...region, id: 12, x: 0, active: false },
    ]),
  ).toEqual([7, 8]);
});

test("the whole atlas fits a bounded cluster view and close zoom uses smaller buckets", () => {
  const world = { minX: 0, minZ: 0, maxX: 96_000, maxZ: 96_000 };
  const far = resourceClusterLevel(world, 500);
  const overview = resourceClusterBounds(world, far);
  expect(resourceClusterBoundsFit(overview)).toBe(true);
  expect(resourceClusterWorldBounds(overview, far)).toEqual(world);
  const close = resourceClusterLevel({ minX: 0, minZ: 0, maxX: 500, maxZ: 500 }, 4);
  expect(close).toBeLessThan(far);
  const left = resourceClusterExtent(1, 2, close);
  const right = resourceClusterExtent(2, 2, close);
  expect(left[2]).toBe(right[0]);
});
