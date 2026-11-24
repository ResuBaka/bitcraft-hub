import type { Watchtower } from "~/types/Watchtower";

type EnergyState = Pick<Watchtower, "active" | "energy" | "upkeep" | "under_siege" | "decay_tick_millis">;

export function watchtowerEmptyBy(tower: EnergyState, fetchedAt: number | null): Date | string {
  if (tower.energy <= 0) return "Depleted";
  if (!tower.active) return "Not draining (inactive)";
  if (tower.under_siege) return "Paused during siege";
  if (tower.upkeep <= 0) return "No upkeep";
  if (!tower.decay_tick_millis || tower.decay_tick_millis < 0 || fetchedAt === null) return "Estimate unavailable";
  const deadline = fetchedAt + Math.ceil(tower.energy / tower.upkeep) * tower.decay_tick_millis;
  // Round up to the displayed minute to preserve the upper bound.
  const date = new Date(Math.ceil(deadline / 60_000) * 60_000);
  return Number.isFinite(date.getTime()) ? date : "Estimate unavailable";
}
