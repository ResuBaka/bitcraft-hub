import { expect, test } from "bun:test";
import { claimTabSlug, getClaimTab, getLegacyClaimTab } from "../app/utils/claimTabs";

test("claim tabs use path slugs and the base claim URL opens Members", () => {
  expect(getClaimTab(undefined)).toBe("members");
  expect(claimTabSlug("members")).toBe("");
  expect(getClaimTab("crafting")).toBe("crafting");
  expect(getClaimTab("xp-history")).toBe("xp_history");
  expect(claimTabSlug("player_offline_tools")).toBe("player-offline-tools");
  expect(getClaimTab("wrong-tab")).toBeUndefined();
});

test("legacy tab values resolve to the canonical claim paths", () => {
  const legacy = getLegacyClaimTab("inventory_changelogs");
  expect(legacy && claimTabSlug(legacy)).toBe("inventory-changelogs");
  expect(getLegacyClaimTab("crafting")).toBe("crafting");
  expect(getLegacyClaimTab(["xp_history", "members"])).toBeUndefined();
  expect(getLegacyClaimTab("wrong-tab")).toBeUndefined();
});
