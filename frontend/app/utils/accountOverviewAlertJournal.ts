import { overviewAlertTypes, type AlertObservation, type AlertType } from "./accountOverviewAlerts";
import type { TrackedPlayer } from "./accountOverviewShare";
import { formatNumber } from "./formatNumber";

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
  passiveGroups?: PassiveGroup[];
};
type PassiveGroup = {
  player: TrackedPlayer;
  until: number;
  id: string;
  recipes: { name: string; count: number }[];
};
const day = 86_400_000;
const maxEntries = 200;
export const emptyJournal = (): AlertJournal => ({ marks: {}, entries: [] });

export function pruneJournal(journal: AlertJournal, now: number) {
  journal.entries = journal.entries.filter((e) => e.time > now - 7 * day).slice(0, maxEntries);
  if (journal.passiveGroups)
    journal.passiveGroups = journal.passiveGroups.filter((g) => g.until > now - 7 * day);
  // Delivery receipts outlive the visible history (including manual clears).
  for (const [key, mark] of Object.entries(journal.marks)) {
    if (mark.time <= now - 30 * day) delete journal.marks[key];
  }
}

function appendEntry(journal: AlertJournal, entry: AlertEntry) {
  journal.entries.unshift(entry);
  if (journal.entries.length > maxEntries) journal.entries.length = maxEntries;
  return entry;
}

// Use the first craft's duration so staggered completions cannot keep postponing the alert.
function passiveWindowMs(seconds: number | null) {
  return Math.max(
    10_000,
    Math.min(120_000, Number.isFinite(seconds) ? Number(seconds) * 1000 * 0.05 : 0),
  );
}

export function flushPassiveGroups(
  journal: AlertJournal,
  now: number,
  // null: this tab does not monitor this player. Leave their batch to its owner.
  allowed: (playerId: string) => boolean | null,
): AlertEntry[] {
  const ready: AlertEntry[] = [];
  if (!journal.passiveGroups) return ready;
  journal.passiveGroups = journal.passiveGroups.filter((group) => {
    const enabled = allowed(group.player.id);
    if (enabled === null) return true;
    if (!enabled) return false;
    if (group.until > now) return true;
    const total = group.recipes.reduce((n, recipe) => n + recipe.count, 0);
    const names = group.recipes
      .slice(0, 3)
      .map((r) => (r.count > 1 ? `${formatNumber(r.count)} × ${r.name}` : r.name));
    if (group.recipes.length > names.length)
      names.push(`+${formatNumber(group.recipes.length - names.length)} more recipes`);
    const message =
      total === 1
        ? `${names[0]} is ready to collect.`
        : `${formatNumber(total)} passive crafts ready to collect: ${names.join(", ")}.`;
    ready.push(
      appendEntry(journal, {
        id: group.id,
        time: now,
        player: group.player,
        type: "passive-ready",
        message,
      }),
    );
    return false;
  });
  return ready;
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
  if (obs.type === "passive-ready") {
    const groups = (journal.passiveGroups ??= []);
    let group = groups.find((g) => g.player.id === player.id && now < g.until);
    if (!group) {
      group = {
        player: { id: player.id, name: player.name },
        until: now + passiveWindowMs(obs.durationSeconds),
        id: `${key}:${now}`,
        recipes: [],
      };
      groups.push(group);
    }
    const recipe = group.recipes.find((r) => r.name === obs.recipe);
    if (recipe) recipe.count++;
    else group.recipes.push({ name: obs.recipe, count: 1 });
    return;
  }
  return appendEntry(journal, {
    id: `${key}:${now}`,
    time: now,
    player: { id: player.id, name: player.name },
    type: obs.type,
    message: obs.message ?? overviewAlertTypes[obs.type].label,
  });
}
