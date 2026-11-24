import type { PlayerCraft } from "../types/PlayerCraft";
import type { AccountOverviewBlockId } from "./accountOverviewBlocks";
import { craftMatchesClaims } from "./craftClaimFilter";
import type { StatsSnapshot } from "../types/StatsSnapshot";
import type { CurrentActivity } from "./playerActivity";

export const overviewAlertTypes = {
  "food-expired": { label: "Food buff expired", block: "stats" },
  "activity-stopped": {
    label: "Idle for 20 seconds after crafting or gathering",
    block: "activity",
  },
  "passive-ready": { label: "Passive craft ready to collect", block: "passive-crafts" },
  "low-health": { label: "Health at or below 25%", block: "stats" },
  "full-stamina": { label: "Stamina full again", block: "stats" },
} as const satisfies Record<string, { label: string; block: AccountOverviewBlockId }>;
export type AlertType = keyof typeof overviewAlertTypes;
export type AlertObservation = (
  | { type: "low-health"; active: boolean }
  | { type: "full-stamina"; active: boolean; suppressed: boolean }
  | { type: "activity-stopped" | "food-expired" | "passive-ready"; eventId: string }
) & { message?: string };

export type AlertPreferences = {
  enabled: boolean;
  volume: number; // Percent, shared by global and player sounds.
  enabledTypes: AlertType[];
  mutedPlayers: string[];
  mutedBlocks: Record<string, AccountOverviewBlockId[]>;
};

export function toggleAlertMute(
  prefs: AlertPreferences,
  player: string,
  block?: AccountOverviewBlockId,
) {
  if (block) {
    const muted = prefs.mutedBlocks[player] ?? [];
    prefs.mutedBlocks[player] = muted.includes(block)
      ? muted.filter((id) => id !== block)
      : [...muted, block];
  } else {
    prefs.mutedPlayers = prefs.mutedPlayers.includes(player)
      ? prefs.mutedPlayers.filter((id) => id !== player)
      : [...prefs.mutedPlayers, player];
  }
}

export function alertsAllowed(prefs: AlertPreferences, player: string, type: AlertType) {
  return (
    prefs.enabled &&
    prefs.enabledTypes.includes(type) &&
    !prefs.mutedPlayers.includes(player) &&
    !prefs.mutedBlocks[player]?.includes(overviewAlertTypes[type].block)
  );
}

export function passiveReadyAlerts(
  previous: PlayerCraft[] | null | undefined,
  current: PlayerCraft[] | null | undefined,
  claims: string[],
): AlertObservation[] {
  if (!previous || !current || previous === current) return [];
  const ready = (c: PlayerCraft) => c.kind === "Passive" && c.status === "Complete";
  const before = new Set(previous.filter(ready).map((c) => c.id));
  return current
    .filter((c) => ready(c) && !before.has(c.id) && craftMatchesClaims(c, claims))
    .map((c) => ({
      type: "passive-ready",
      eventId: c.id,
      message: `${c.recipe_name ?? `Recipe #${c.recipe_id}`} is ready to collect.`,
    }));
}

type VitalObservation = Extract<AlertObservation, { active: boolean }>;
function hasStaminaRegenBuff(stats: StatsSnapshot) {
  return (
    stats.buffs?.some(
      (buff) =>
        !buff.paused &&
        (buff.remaining_seconds == null || buff.remaining_seconds > 0) &&
        (buff.stats?.length
          ? buff.stats.some(
              (stat) =>
                stat.value > 0 &&
                (stat.name === "ActiveStaminaRegenRate" || stat.name === "PassiveStaminaRegenRate"),
            )
          : buff.name === "Stamina Regen"),
    ) ?? false
  );
}

export function vitalAlerts(
  stats: StatsSnapshot,
  activity: CurrentActivity["recipeKind"] = null,
): VitalObservation[] {
  if (!stats.available) return [];
  const alerts: VitalObservation[] = [];
  const { health, max_health, stamina, max_stamina } = stats;
  if (health != null && max_health != null && max_health > 0)
    alerts.push({
      type: "low-health",
      active: health <= max_health * 0.25,
      message: `Low health: ${health} / ${max_health}`,
    });
  if (stamina != null && max_stamina != null && max_stamina > 0)
    alerts.push({
      type: "full-stamina",
      active: stamina >= max_stamina,
      suppressed: activity === "craft" || activity === "gather" || hasStaminaRegenBuff(stats),
    });
  return alerts;
}

// The display lingers briefly between actions. Time this alert from the raw
// active -> idle transition, and discard a pending stop on disconnect/sign-out.
export class IdleAlertTimer {
  private action?: string;
  private idleSince?: number;
  private recovering = false;
  update(
    activity: CurrentActivity | null,
    now: number,
    stats?: Pick<StatsSnapshot, "available" | "stamina" | "max_stamina"> | null,
    staminaCost = 0,
  ): AlertObservation | undefined {
    const working =
      activity?.actionStart != null &&
      (activity.recipeKind === "craft" || activity.recipeKind === "gather");
    const action = working
      ? String(activity.actionStart)
      : activity?.label === "Idle"
        ? this.action
        : undefined;
    if (action !== this.action) {
      this.action = action;
      this.recovering = false;
      this.idleSince = undefined;
    }
    if (action === undefined) return;
    if (stats?.available && stats.stamina != null) {
      if (stats.max_stamina != null && stats.max_stamina > 0 && stats.stamina >= stats.max_stamina)
        this.recovering = false;
      else if (stats.stamina <= 0 || stats.stamina < staminaCost) this.recovering = true;
    }
    if (working || this.recovering) {
      this.idleSince = undefined;
      return;
    }
    this.idleSince ??= now;
    if (now - this.idleSince >= 20_000) {
      this.action = undefined;
      this.idleSince = undefined;
      return { type: "activity-stopped", eventId: action };
    }
  }
}

export class FoodExpiryTracker {
  private timers = new Map<string, { end: number; name: string | null }>();

  snapshot(stats: StatsSnapshot, now: number): AlertObservation[] {
    const old = this.timers;
    this.timers = new Map();
    if (!stats.available || !stats.food_metadata_available || !stats.buffs) return [];
    for (const buff of stats.buffs) {
      if (
        !buff.is_food_associated ||
        !(buff.duration > 60) ||
        buff.paused ||
        buff.remaining_seconds == null
      )
        continue;
      this.timers.set(`${buff.buff_id}:${buff.start_timestamp}`, {
        end: now + Number(buff.remaining_seconds) * 1000,
        name: buff.name,
      });
    }
    // A server snapshot can remove an expired buff just before our next tick.
    const expired: AlertObservation[] = [];
    if (stats.buffs_complete) {
      for (const [id, timer] of old) {
        if (
          !this.timers.has(id) &&
          timer.end <= now + 1000 &&
          !stats.buffs.some((b) => `${b.buff_id}:${b.start_timestamp}` === id)
        )
          expired.push(this.expired(id, timer.name));
      }
    }
    return expired;
  }

  tick(now: number): AlertObservation[] {
    const expired: AlertObservation[] = [];
    for (const [id, timer] of this.timers) {
      if (timer.end > now) continue;
      this.timers.delete(id);
      expired.push(this.expired(id, timer.name));
    }
    return expired;
  }
  private expired(eventId: string, name: string | null): AlertObservation {
    return {
      type: "food-expired",
      eventId,
      message: name ? `Food buff expired: ${name}` : undefined,
    };
  }
}
