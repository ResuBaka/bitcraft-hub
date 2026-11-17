import type { ClaimTechDesc } from "../types/ClaimTechDesc";

export function getClaimMaxSupplies(upgrades: ClaimTechDesc[], learned: number[]): number | null {
  if (!learned.length) return null;

  const upgradesById = new Map(upgrades.map((upgrade) => [upgrade.id, upgrade]));
  let maxSupplies = -Infinity;
  for (const id of learned) {
    const upgrade = upgradesById.get(id);
    if (!upgrade) return null;
    maxSupplies = Math.max(maxSupplies, upgrade.supplies);
  }
  return maxSupplies;
}
