import type { PlayerCraft } from "~/types/PlayerCraft";

type PassiveCraft = Pick<
  PlayerCraft,
  "id" | "recipe_id" | "recipe_name" | "outputs" | "status" | "building_id"
> &
  Partial<Pick<PlayerCraft, "timestamp" | "duration_seconds">>;

export type PassiveCraftSummary = {
  key: string;
  name: string;
  item: PlayerCraft["outputs"][number] | null;
  ready: number;
  readyIds: string[];
  total: number;
  buildingId: string | null;
  nextFinish: number | null;
  unknownProcessing: number;
  queued: number;
};

export function summarizePassiveCrafts(crafts: PassiveCraft[]): PassiveCraftSummary[] {
  const groups = new Map<string, PassiveCraftSummary>();
  for (const craft of crafts) {
    const outputs = craft.outputs?.length ? craft.outputs : [null];
    const seen = new Set<string>();
    for (const item of outputs) {
      const key = item
        ? JSON.stringify(["item", item.name, item.icon_asset_name, item.tier])
        : JSON.stringify(["recipe", craft.recipe_id]);
      if (seen.has(key)) continue;
      seen.add(key);
      let group = groups.get(key);
      if (!group) {
        group = {
          key,
          name: item?.name ?? craft.recipe_name ?? `Recipe #${craft.recipe_id}`,
          item,
          ready: 0,
          readyIds: [],
          total: 0,
          buildingId: craft.building_id,
          nextFinish: null,
          unknownProcessing: 0,
          queued: 0,
        };
        groups.set(key, group);
      } else if (group.buildingId !== craft.building_id) {
        group.buildingId = null;
      }
      group.total++;
      if (craft.status === "Complete") {
        group.ready++;
        group.readyIds.push(craft.id);
      }
      if (craft.status === "Queued") group.queued++;
      if (craft.status === "Processing") {
        const start = craft.timestamp ? Date.parse(craft.timestamp) : NaN;
        const duration = craft.duration_seconds;
        const finish =
          duration != null && Number.isFinite(duration) && duration >= 0
            ? start + duration * 1000
            : NaN;
        if (Number.isFinite(finish)) {
          group.nextFinish = Math.min(group.nextFinish ?? Infinity, finish);
        } else {
          group.unknownProcessing++;
        }
      }
    }
  }
  return [...groups.values()];
}

export function passiveSummaryTiming(
  summary: PassiveCraftSummary,
  now: number,
): { label: string; detail: string } {
  if (summary.nextFinish !== null) {
    if (summary.nextFinish <= now)
      return {
        label: "Finishing",
        detail: "Estimated finish reached; awaiting server confirmation.",
      };
    let seconds = Math.ceil((summary.nextFinish - now) / 1000);
    const days = Math.floor(seconds / 86400);
    seconds %= 86400;
    const hours = Math.floor(seconds / 3600);
    seconds %= 3600;
    const minutes = Math.floor(seconds / 60);
    seconds %= 60;
    const time = [
      days && `${days}d`,
      hours && `${hours}h`,
      minutes && `${minutes}m`,
      seconds && `${seconds}s`,
    ]
      .filter(Boolean)
      .join(" ");
    const unknown = summary.unknownProcessing ? " Some project times are unknown." : "";
    const queued = summary.queued ? " Queued projects are not included." : "";
    return { label: time, detail: `Estimated time until the next finish.${unknown}${queued}` };
  }
  if (summary.unknownProcessing)
    return { label: "Time unknown", detail: "Processing time is unavailable." };
  if (summary.queued) return { label: "Queued", detail: "Waiting for an available slot." };
  return {
    label: summary.ready === summary.total ? "Done" : "—",
    detail:
      summary.ready === summary.total
        ? "All projects are ready to collect."
        : "Finish time is unavailable.",
  };
}
