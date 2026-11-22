import type { CharacterStatsDelta } from "../types/CharacterStatsDelta";
import type { CharacterStatsSnapshot } from "../types/CharacterStatsSnapshot";

// Revisions are server-local unsigned counters; reconnects get a new baseline.
export function isNewerCharacterStatsRevision(revision: number, base: number): boolean {
  const distance = (revision - base) >>> 0;
  return distance > 0 && distance < 0x80000000;
}

export function applyCharacterStatsDelta(
  current: CharacterStatsSnapshot | null,
  delta: CharacterStatsDelta,
): { status: "ignored" | "resync" } | { status: "applied"; snapshot: CharacterStatsSnapshot } {
  if (current?.revision == null || !current.values) return { status: "resync" };
  if (!isNewerCharacterStatsRevision(delta.revision, current.revision))
    return { status: "ignored" };
  if (delta.base_revision !== current.revision) return { status: "resync" };
  const values = current.values.slice();
  for (const [index, value] of delta.changes) {
    if (!Number.isInteger(index) || index < 0 || index >= values.length)
      return { status: "resync" };
    values[index] = value;
  }
  return { status: "applied", snapshot: { ...current, revision: delta.revision, values } };
}
