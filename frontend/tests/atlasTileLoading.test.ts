import { expect, test } from "bun:test";
import { loadAtlasZoomTiles } from "../app/utils/atlasTileLoading";

test("zoom tile bursts reuse the queue and respect its concurrency and frame budgets", () => {
  const calls: (string | number[])[] = [];
  let loading = 0;
  let empty = false;
  const queue = {
    isEmpty: () => empty,
    getTilesLoading: () => loading,
    reprioritize: () => calls.push("reprioritize"),
    loadMoreTiles: (total: number, next: number) => calls.push([total, next]),
  };
  loadAtlasZoomTiles(queue, 1_000, 1_003);
  expect(calls).toEqual(["reprioritize", [12, 6]]);

  loadAtlasZoomTiles(queue, 1_000, 1_020);
  loading = 12;
  loadAtlasZoomTiles(queue, 1_000, 1_003);
  loading = 0;
  empty = true;
  loadAtlasZoomTiles(queue, 1_000, 1_003);
  expect(calls).toHaveLength(2);
});
