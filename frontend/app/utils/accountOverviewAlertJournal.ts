import { overviewAlertTypes, type AlertObservation, type AlertType } from "./accountOverviewAlerts";
import type { TrackedPlayer } from "./accountOverviewShare";

export type AlertEntry = {
  player: TrackedPlayer;
  type: AlertType;
  message: string;
  time: number;
  id: string;
};
export type AlertJournal = {
  marks: Record<string, { active: boolean; time: number }>;
  entries: AlertEntry[];
};
const day = 86_400_000;
const maxEntries = 200;
export const emptyJournal = (): AlertJournal => ({ marks: {}, entries: [] });

export function pruneJournal(journal: AlertJournal, now: number) {
  journal.entries = journal.entries.filter((e) => e.time > now - 7 * day).slice(0, maxEntries);
  // Delivery receipts outlive the visible history (including manual clears).
  for (const [key, mark] of Object.entries(journal.marks)) {
    if (mark.time <= now - 30 * day) delete journal.marks[key];
  }
}

export function recordObservation(
  journal: AlertJournal,
  player: TrackedPlayer,
  obs: AlertObservation,
  enabled: boolean,
  now: number,
): AlertEntry | undefined {
  const key = JSON.stringify([player.id, obs.type, "eventId" in obs ? obs.eventId : null]);
  const previous = journal.marks[key];
  const active = "active" in obs ? obs.active : true;
  if (previous?.active === active) return;
  journal.marks[key] = { active, time: now };
  // Consume this refill even if it happened during work or a regen buff.
  // Ending the activity/buff while still full is not another refill.
  if (obs.type === "full-stamina" && obs.suppressed) return;
  // For meters, the first observation is a baseline, not a crossing.
  if (!enabled || !active || ("active" in obs && !previous)) return;
  const entry = {
    id: `${key}:${now}`,
    time: now,
    player: { id: player.id, name: player.name },
    type: obs.type,
    message: obs.message ?? overviewAlertTypes[obs.type].label,
  };
  journal.entries.unshift(entry);
  if (journal.entries.length > maxEntries) journal.entries.length = maxEntries;
  return entry;
}
