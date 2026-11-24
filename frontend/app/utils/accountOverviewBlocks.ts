export const accountOverviewBlocks = [
  {
    id: "stats",
    name: "Stats",
    description: "Health, stamina and buffs",
    icon: "i-lucide-heart-pulse",
  },
  {
    id: "activity",
    name: "Activity",
    description: "Current player activity",
    icon: "i-lucide-activity",
  },
  {
    id: "active-crafts",
    name: "Active Crafts",
    description: "Active crafting projects",
    icon: "i-lucide-hammer",
  },
  {
    id: "passive-crafts",
    name: "Passive Crafts",
    description: "Passive crafting projects",
    icon: "i-lucide-clock",
  },
  {
    id: "placeables",
    name: "Placeables",
    description: "Owned placeables, growth timers and interactions",
    icon: "i-lucide-sprout",
  },
] as const;

export type AccountOverviewBlockId = (typeof accountOverviewBlocks)[number]["id"];
export type AddBlockRequest = {
  playerId: string;
  blockId: AccountOverviewBlockId;
  sequence: number;
};
export const blockDragType = "application/x-bitcraft-account-block";

export function isAccountOverviewBlockId(value: string): value is AccountOverviewBlockId {
  return accountOverviewBlocks.some((block) => block.id === value);
}

export function blockDropPosition(
  x: number,
  y: number,
  rect: { left: number; top: number; width: number },
  width = 6,
): { x: number; y: number } {
  const column = rect.width > 0 ? Math.floor(((x - rect.left) / rect.width) * 12) : 0;
  return {
    x: Math.max(0, Math.min(12 - width, column)),
    y: Math.max(0, Math.floor((y - rect.top) / 40)),
  };
}
