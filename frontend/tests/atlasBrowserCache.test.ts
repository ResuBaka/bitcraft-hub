import { expect, test } from "bun:test";
import { atlasCacheKey, withAtlasBrowserCache } from "../app/utils/atlasBrowserCache";

const base = "https://atlas.example/map";
const endpoint = "https://api.example/world-map/roads";

test("atlas cache keys are scoped to the API and canonicalized by query", () => {
  expect(atlasCacheKey(endpoint, { z: 9, x: 1 }, base)).toBe(`${endpoint}?x=1&z=9`);
  expect(atlasCacheKey("/world-map/roads", { x: 1, z: 9 }, base)).not.toBe(
    atlasCacheKey(endpoint, { x: 1, z: 9 }, base),
  );
  expect(atlasCacheKey(endpoint, { ids: "1,2" }, base)).not.toBe(
    atlasCacheKey(endpoint, { ids: "1" }, base),
  );
});

test("saved data renders before the API finishes and fresh data replaces the stored copy", async () => {
  const refreshed = Promise.withResolvers<string>();
  const preview = Promise.withResolvers<string>();
  const written = Promise.withResolvers<unknown>();
  const fetch = withAtlasBrowserCache(endpoint, base, () => refreshed.promise, {
    read: async () => "saved",
    write: async (_key, value) => written.resolve(value),
  });
  const result = fetch({ x: 1 }, new AbortController().signal, preview.resolve);
  expect(await preview.promise).toBe("saved");
  refreshed.resolve("fresh");
  expect(await result).toBe("fresh");
  expect(await written.promise).toBe("fresh");
});

test("a late cache read cannot replace fresh data or publish after cancellation", async () => {
  for (const cancel of [false, true]) {
    const saved = Promise.withResolvers<string>();
    const controller = new AbortController();
    const previews: string[] = [];
    const fetch = withAtlasBrowserCache(endpoint, base, async () => "fresh", {
      read: () => saved.promise,
      write: async () => {},
    });
    const result = fetch({}, controller.signal, (data) => previews.push(data));
    if (cancel) controller.abort();
    expect(await result).toBe("fresh");
    saved.resolve("saved");
    await saved.promise;
    expect(previews).toEqual([]);
  }
});
