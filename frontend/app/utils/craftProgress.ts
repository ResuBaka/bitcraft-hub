type CraftProgress = { id: string; kind: string; progress: number | null; progress_total: number | bigint | null; status: string; timestamp: string | null };
type ProgressUpdate = { craft_id: string; progress: number; lock_expiration: string };

export function applyCraftProgress<T extends CraftProgress>(crafts: T[], update: ProgressUpdate, now = Date.now()): T[] | null {
  const index = crafts.findIndex((craft) => craft.kind === "Active" && craft.id === update.craft_id);
  if (index < 0) return null;
  const craft = crafts[index]!;
  const expiration = Date.parse(update.lock_expiration);
  const status = craft.progress_total === null ? "Unknown"
    : update.progress >= Number(craft.progress_total) ? "Complete"
      : Number.isFinite(expiration) && expiration <= now ? "Suspended" : "Active";
  const result = crafts.slice();
  result[index] = { ...craft, progress: update.progress, timestamp: update.lock_expiration, status };
  return result;
}

export function applyPassiveCraftUpdate<T extends { id: string; kind: string; status: string; slot: number | null; timestamp: string | null }>(crafts: T[], update: { craft_id: string; status: string; slot: number | null; timestamp: string }): T[] | null {
  const index = crafts.findIndex((craft) => craft.kind === "Passive" && craft.id === update.craft_id);
  if (index < 0) return null;
  const result = crafts.slice();
  result[index] = { ...crafts[index]!, status: update.status, slot: update.slot, timestamp: update.timestamp };
  return result;
}
