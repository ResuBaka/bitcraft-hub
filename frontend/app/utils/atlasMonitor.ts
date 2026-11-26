export type MonitorKind = "player" | "claim";
export type MonitorTarget = { kind: MonitorKind | "resource"; id: string; name: string };
export type MonitorMember = { id: string; name: string };
export type MonitorRosters = Record<string, MonitorMember[]>;
export type MonitorPreferences = { tracking: MonitorTarget[]; favorites: MonitorTarget[] };

export const ATLAS_MONITOR_KEY = "bitcraft-atlas-monitor-v1";
export const MAX_TRACKED_TARGETS = 20;
export const MAX_FAVORITES = 50;

export function monitorKey({ kind, id }: MonitorTarget) {
  return `${kind}:${id}`;
}

export function monitorEntityId(value: unknown): value is string {
  return (
    typeof value === "string" &&
    /^[1-9]\d{0,19}$/.test(value) &&
    (value.length < 20 || value <= "18446744073709551615")
  );
}

function monitorResourceId(value: unknown): value is string {
  if (typeof value !== "string" || !/^(0|-?[1-9]\d{0,9})$/.test(value)) return false;
  const id = Number(value);
  return id >= -2_147_483_648 && id <= 2_147_483_647;
}

function readTargets(value: unknown, limit: number, allowResources = false): MonitorTarget[] {
  if (!Array.isArray(value)) return [];
  const targets = new Map<string, MonitorTarget>();
  for (const row of value) {
    if (targets.size === limit) break;
    const valid =
      row?.kind === "resource"
        ? allowResources && monitorResourceId(row.id)
        : (row?.kind === "player" || row?.kind === "claim") && monitorEntityId(row.id);
    if (!valid || typeof row.name !== "string") continue;
    const target: MonitorTarget = { id: row.id, kind: row.kind, name: row.name.slice(0, 160) };
    targets.set(monitorKey(target), target);
  }
  return [...targets.values()];
}

export function readAtlasMonitor(raw: string): MonitorPreferences {
  try {
    const data = JSON.parse(raw);
    return {
      tracking: readTargets(data?.tracking, MAX_TRACKED_TARGETS),
      favorites: readTargets(data?.favorites, MAX_FAVORITES, true),
    };
  } catch {
    return { tracking: [], favorites: [] };
  }
}

export function trackedAtlasPlayers(targets: readonly MonitorTarget[], rosters: MonitorRosters) {
  const players = new Map<string, string>();
  for (const target of targets) {
    if (target.kind !== "claim") continue;
    for (const { id, name } of rosters[target.id] ?? []) {
      if (name || !players.has(id)) players.set(id, name);
    }
  }
  for (const target of targets) {
    if (target.kind === "player" && (target.name || !players.has(target.id))) {
      players.set(target.id, target.name);
    }
  }
  return players;
}
