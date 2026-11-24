import type { WatchtowerObservation } from "~/types/WatchtowerObservation";
import type { WatchtowerReading } from "~/types/WatchtowerReading";
import { empireLocation } from "./empireLocation";

const owner = (s: WatchtowerReading) => s.empire_name || `Empire ${s.empire_id}`;
const number = (n: number) => n.toLocaleString("en-US");
const location = (s: WatchtowerReading) => s.x == null || s.z == null ? "Unknown" : empireLocation(s.x, s.z);

export function describeObservation(row: WatchtowerObservation): { title: string; details: string[] } {
  const a = row.previous, b = row.current;
  const siege = row.source === "siege";
  const subject = siege ? "Siege side" : "Watchtower";
  const state = b ?? a;
  if (!state) return { title: subject, details: [] };
  const status = (s: WatchtowerReading) => siege
    ? (s.active ? "Siege active" : "Marked for siege")
    : (s.active ? "Active" : "Inactive");
  if (!a || !b) {
    const title = row.event === "baseline" ? `${subject} baseline`
      : !b ? (siege ? (a?.active ? "Siege side removed" : "Siege mark removed") : "Tower no longer present")
      : `${subject} first observed`;
    const details = [owner(state), status(state), `Energy ${number(state.energy)}`];
    if (state.upkeep != null) details.push(`Upkeep ${number(state.upkeep)}`);
    if (state.x != null && state.z != null) details.push(`Location ${location(state)}`);
    return { title, details };
  }
  const details: string[] = [];
  if (a.empire_id !== b.empire_id) details.push(`${owner(a)} → ${owner(b)}`);
  else if (siege) details.push(owner(b));
  if (a.active !== b.active) details.push(`${status(a)} → ${status(b)}`);
  if (a.energy !== b.energy) details.push(`Energy ${number(a.energy)} → ${number(b.energy)}`);
  if (a.upkeep !== b.upkeep) details.push(`Upkeep ${a.upkeep} → ${b.upkeep}`);
  if (location(a) !== location(b) || a.dimension !== b.dimension)
    details.push(`Location ${location(a)} (D${a.dimension}) → ${location(b)} (D${b.dimension})`);
  const title = !siege && a.empire_id !== b.empire_id ? "Ownership changed"
    : a.active !== b.active ? (siege ? status(b) : (b.active ? "Tower activated" : "Tower deactivated"))
    : `${subject} updated`;
  return { title, details };
}
