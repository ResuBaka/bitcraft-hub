const claimTabs = [
  "members",
  "building_items",
  "player_items",
  "player_tools",
  "player_offline_items",
  "player_offline_tools",
  "buildings",
  "crafting",
  "leaderboards",
  "xp_history",
  "upgrades",
  "inventory_changelogs",
  "events",
  "traveler_tasks",
  "marketplace",
] as const;

export type ClaimTab = (typeof claimTabs)[number];

export function claimTabSlug(tab: ClaimTab): string {
  return tab === "members" ? "" : tab.replaceAll("_", "-");
}

export function getClaimTab(slug: string | string[] | undefined): ClaimTab | undefined {
  return claimTabs.find((tab) => claimTabSlug(tab) === (slug ?? ""));
}

export function getLegacyClaimTab(query: unknown): ClaimTab | undefined {
  return claimTabs.find((tab) => tab === query);
}
