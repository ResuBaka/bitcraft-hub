import type { PlayerActivitySnapshot } from "~/types/PlayerActivitySnapshot";
import type { PlayerCraft } from "~/types/PlayerCraft";

export type CurrentActivity = {
  actionStart?: number;
  mountId?: string;
  detail?: string;
  label: string;
  icon: string;
  recipeKind: "craft" | "gather" | null;
  recipeId: number | null;
  targetId: string | null;
  craftOwnerId?: string;
  craftId?: string;
};

export function currentPlayerActivity(
  snapshot: PlayerActivitySnapshot | null,
  now: number,
): CurrentActivity {
  let actionStart: number | undefined;
  const result = (
    label: string,
    icon: string,
    recipeKind: CurrentActivity["recipeKind"] = null,
    recipeId: number | null = null,
    targetId: string | null = null,
  ): CurrentActivity => ({
    label,
    icon,
    recipeKind,
    recipeId,
    targetId,
    actionStart,
    mountId: snapshot?.mount?.entity_id,
  });
  if (!snapshot) return result("Loading…", "i-lucide-loader-circle");
  if (!snapshot.signed_in) return result("Offline", "i-lucide-circle-off");
  if (!snapshot.available || snapshot.actions === null)
    return result("Activity unavailable", "i-lucide-circle-help");
  const isWork = (type: string) => type === "Craft" || type === "Extract";
  const active = snapshot.actions.filter((action) => {
    const start = Number(action.start_ms);
    const duration = Number(action.duration_ms);
    return (
      action.action_type !== "None" &&
      action.successful &&
      !action.consumed &&
      Number.isFinite(start) &&
      Number.isFinite(duration) &&
      duration > 0 &&
      start <= now + (isWork(action.action_type) ? 1000 : 500) &&
      start + duration > now
    );
  });
  const priority = (type: string) =>
    isWork(type) ? 2 : type === "PlayerMove" || type === "DeployableMove" ? 0 : 1;
  active.sort(
    (a, b) =>
      priority(b.action_type) - priority(a.action_type) || Number(b.start_ms) - Number(a.start_ms),
  );
  // Hit timestamps come from the game client. BitCraft accepts requests up to
  // 8s late or 1s early, so a valid hit can already look expired on arrival.
  // For consumed hits the game moves start_ms to approximately the hit's end.
  // Only bridge a freshly sampled hit; ticking an unchanged snapshot must
  // still lead to Idle, and a genuinely active action always wins.
  const sampled = Number(snapshot.server_time_ms);
  const action =
    active[0] ??
    snapshot.actions
      .filter((a) => {
        const ageAtSample = sampled - Number(a.start_ms);
        const sinceSample = now - sampled;
        return (
          isWork(a.action_type) &&
          a.successful &&
          ageAtSample >= -1000 &&
          ageAtSample <= 8000 &&
          sinceSample >= 0 &&
          sinceSample < Math.min(Number(a.duration_ms), 1500)
        );
      })
      .sort((a, b) => Number(b.start_ms) - Number(a.start_ms))[0];
  const mount = snapshot.mount;
  const mountName = mount?.name?.trim() || `a ${mount?.kind?.toLowerCase() || "deployable"}`;
  if (!action) {
    const idle = result("Idle", "i-lucide-pause");
    return mount ? { ...idle, detail: `On ${mountName}` } : idle;
  }
  actionStart = Number(action.start_ms);
  if (action.action_type === "Craft")
    return {
      ...result("Crafting", "i-lucide-hammer", "craft", action.recipe_id, action.target_id ?? null),
      craftOwnerId: action.craft_owner_id ?? undefined,
      craftId: action.craft_id ?? undefined,
    };
  if (action.action_type === "Extract")
    return result(
      "Gathering",
      "i-lucide-pickaxe",
      "gather",
      action.recipe_id,
      action.target_id ?? null,
    );
  if (mount && (action.action_type === "PlayerMove" || action.action_type === "DeployableMove")) {
    if (mount.kind === "Boat")
      return result(`${mount.driver ? "Sailing" : "Aboard"} ${mountName}`, "i-lucide-ship");
    if (mount.kind === "Cart" && mount.driver)
      return result(`Pulling ${mountName}`, "i-lucide-shopping-cart");
    if (mount.kind === "Mount" || !mount.driver)
      return result(`Riding ${mountName}`, "i-lucide-route");
    return result(`Using ${mountName}`, "i-lucide-route");
  }
  if (action.action_type === "PlayerMove")
    return result(
      snapshot.walking === null ? "Moving" : snapshot.walking ? "Walking" : "Running",
      "i-lucide-footprints",
    );
  if (action.action_type === "DeployableMove") return result("Traveling", "i-lucide-footprints");
  return result(action.action_type.replace(/([a-z0-9])([A-Z])/g, "$1 $2"), "i-lucide-activity");
}

/** Keep the last action visible briefly while the game transitions between hits. */
export class ActivityLinger {
  private previous: CurrentActivity | null = null;
  private lastSeen = 0;
  constructor(private readonly durationMs = 3000) {}

  update(activity: CurrentActivity, now: number): CurrentActivity {
    if (activity.label === "Idle") {
      if (
        this.previous &&
        this.previous.mountId === activity.mountId &&
        now - this.lastSeen < this.durationMs
      )
        return this.previous;
      this.previous = null;
      return activity;
    }
    if (["Offline", "Activity unavailable", "Loading…"].includes(activity.label)) {
      this.previous = null;
      return activity;
    }
    this.previous = activity;
    this.lastSeen = now;
    return activity;
  }
}

export function activityCraft(activity: CurrentActivity, crafts: PlayerCraft[] | null | undefined) {
  if (activity.recipeKind !== "craft" || activity.targetId === null) return null;
  const matches = crafts?.filter(
    (craft) =>
      craft.kind === "Active" &&
      craft.building_id === activity.targetId &&
      (activity.recipeId === null || craft.recipe_id === activity.recipeId),
  );
  if (activity.craftId) return matches?.find((c) => c.id === activity.craftId) ?? null;
  // Freshly initiated projects don't have a continue-start event. Ignore
  // collected-later completions and suspended projects at the same station.
  if (activity.recipeId === null) return null;
  const active = matches?.filter((c) => c.status === "Active" || c.status === "Unknown");
  return active?.length === 1 ? active[0]! : null;
}

export function activityCraftProgress(craft: PlayerCraft | null) {
  if (craft?.progress == null || craft.progress_total == null) return null;
  const total = Number(craft.progress_total);
  if (!Number.isFinite(craft.progress) || !Number.isFinite(total) || total <= 0) return null;
  return {
    progress: craft.progress,
    total,
    percent: Math.max(0, Math.min(100, (craft.progress / total) * 100)),
  };
}
