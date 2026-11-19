import type { PlayerCraft } from "~/types/PlayerCraft";

type CraftLocation = Pick<PlayerCraft, "building_context">;

export function craftClaimKey(craft: CraftLocation): string {
  if (!craft.building_context) return "unknown";
  return craft.building_context.claim_id === null
    ? "unclaimed"
    : `claim:${craft.building_context.claim_id}`;
}

export function craftMatchesClaims(craft: CraftLocation, selected: string[]): boolean {
  return selected.length === 0 || selected.includes(craftClaimKey(craft));
}

export function craftClaimOptions(crafts: CraftLocation[], selected: string[]) {
  const options = new Map<string, string>();
  for (const craft of crafts) {
    const key = craftClaimKey(craft);
    const context = craft.building_context;
    const label = !context
      ? "Unknown location"
      : context.claim_id === null
        ? "Unclaimed"
        : (context.claim_name ?? `Claim #${context.claim_id}`);
    options.set(key, label);
  }
  for (const key of selected) {
    if (!options.has(key)) {
      options.set(
        key,
        key === "unknown"
          ? "Unknown location"
          : key === "unclaimed"
            ? "Unclaimed"
            : `Claim #${key.replace(/^claim:/, "")}`,
      );
    }
  }
  return [...options]
    .map(([value, label]) => ({ value, label }))
    .sort((a, b) => a.label.localeCompare(b.label));
}
