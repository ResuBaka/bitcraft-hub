type PassiveTiming = { status: string; timestamp: string | null; duration_seconds: number | null };

function duration(milliseconds: number): string {
  let seconds = Math.max(0, Math.ceil(milliseconds / 1000));
  const days = Math.floor(seconds / 86400);
  seconds %= 86400;
  const hours = Math.floor(seconds / 3600);
  seconds %= 3600;
  const minutes = Math.floor(seconds / 60);
  seconds %= 60;
  return [days ? `${days}d` : "", hours ? `${hours}h` : "", minutes ? `${minutes}m` : "", `${seconds}s`].filter(Boolean).join(" ");
}

/** Rows are grouped by status, owner, and recipe, but can have different times. */
export function passiveCraftTiming(crafts: PassiveTiming[], now: number): string {
  const status = crafts[0]?.status;
  if (status === "Queued") return "Queued — waiting for an available slot";
  if (status !== "Processing" && status !== "Complete") return "—";
  const times = crafts.flatMap((craft) => {
    if (!craft.timestamp) return [];
    const timestamp = Date.parse(craft.timestamp);
    if (!Number.isFinite(timestamp)) return [];
    if (status === "Complete") return [timestamp];
    if (craft.duration_seconds == null || !Number.isFinite(craft.duration_seconds) || craft.duration_seconds < 0) return [];
    const finish = timestamp + craft.duration_seconds * 1000;
    return Number.isFinite(finish) ? [finish] : [];
  }).sort((a, b) => a - b);
  if (!times.length) return status === "Complete" ? "Completed — time unknown" : "Time remaining unknown";
  const first = times[0]!;
  const last = times[times.length - 1]!;
  const unknown = times.length < crafts.length ? " (some times unknown)" : "";
  if (status === "Complete") {
    const from = new Date(first).toLocaleString();
    const to = new Date(last).toLocaleString();
    return `Completed: ${from}${first !== last ? ` – ${to}` : ""}${unknown}`;
  }
  if (last <= now) return `Finishing — awaiting server confirmation${unknown}`;
  const from = duration(first - now);
  const to = duration(last - now);
  return `Finishes in ${from}${from !== to ? ` – ${to}` : ""}${first <= now ? " (some awaiting server confirmation)" : ""}${unknown}`;
}
