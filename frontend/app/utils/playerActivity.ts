import type { PlayerActivitySnapshot } from "~/types/PlayerActivitySnapshot";
import type { PlayerCraft } from "~/types/PlayerCraft";

export type CurrentActivity = {
  actionStart?: number;
  label: string;
  icon: string;
  recipeKind: "craft" | "gather" | null;
  recipeId: number | null;
  targetId: string | null;
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
  ): CurrentActivity => ({ label, icon, recipeKind, recipeId, targetId, actionStart });
  if (!snapshot) return result("Loading…", "i-lucide-loader-circle");
  if (!snapshot.signed_in) return result("Offline", "i-lucide-circle-off");
  if (!snapshot.available || snapshot.actions === null)
    return result("Activity unavailable", "i-lucide-circle-help");
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
      start <= now + 500 &&
      start + duration > now
    );
  });
  const priority = (type: string) =>
    type === "Craft" || type === "Extract"
      ? 2
      : type === "PlayerMove" || type === "DeployableMove"
        ? 0
        : 1;
  active.sort(
    (a, b) =>
      priority(b.action_type) - priority(a.action_type) || Number(b.start_ms) - Number(a.start_ms),
  );
  const action = active[0];
  if (!action) return result("Idle", "i-lucide-pause");
  actionStart = Number(action.start_ms);
  if (action.action_type === "Craft")
    return result(
      "Crafting",
      "i-lucide-hammer",
      "craft",
      action.recipe_id,
      action.target_id ?? null,
    );
  if (action.action_type === "Extract")
    return result(
      "Gathering",
      "i-lucide-pickaxe",
      "gather",
      action.recipe_id,
      action.target_id ?? null,
    );
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
      if (this.previous && now - this.lastSeen < this.durationMs) return this.previous;
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

export function activityCraftProgress(
  activity: CurrentActivity,
  crafts: PlayerCraft[] | null | undefined,
) {
  if (activity.recipeKind !== "craft" || activity.targetId === null || activity.recipeId === null)
    return null;
  const matches = (crafts ?? []).filter(
    (craft) =>
      craft.kind === "Active" &&
      craft.building_id === activity.targetId &&
      craft.recipe_id === activity.recipeId,
  );
  if (matches.length !== 1) return null;
  const craft = matches[0]!;
  if (craft.progress == null || craft.progress_total == null) return null;
  const total = Number(craft.progress_total);
  if (!Number.isFinite(craft.progress) || !Number.isFinite(total) || total <= 0) return null;
  return {
    progress: craft.progress,
    total,
    percent: Math.max(0, Math.min(100, (craft.progress / total) * 100)),
  };
}
