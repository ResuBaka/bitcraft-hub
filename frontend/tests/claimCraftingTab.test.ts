import { expect, test } from "bun:test";

test("crafting is an accepted claim tab for URL synchronization", async () => {
  const source = await Bun.file(new URL("../app/pages/claims/[id].vue", import.meta.url)).text();
  const validTabs = source.match(/const validTabs = new Set\(\[([\s\S]*?)\]\)/)?.[1];
  expect(validTabs).toBeDefined();
  expect(validTabs).toContain('"crafting"');
});
