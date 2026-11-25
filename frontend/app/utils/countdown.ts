import { formatNumber } from "./formatNumber";

/** Display partial minutes as whole minutes; keep seconds for the final minute. */
export function formatCountdown(milliseconds: number, largestUnit: "day" | "minute" = "day"): string {
  if (milliseconds < 60_000) return `${formatNumber(Math.max(0, Math.ceil(milliseconds / 1000)))}s`;
  const minutes = Math.ceil(milliseconds / 60_000);
  if (largestUnit === "minute") return `${formatNumber(minutes)}m`;
  const days = Math.floor(minutes / 1440);
  const hours = Math.floor((minutes % 1440) / 60);
  const rest = minutes % 60;
  return [days && `${formatNumber(days)}d`, hours && `${formatNumber(hours)}h`, rest && `${formatNumber(rest)}m`].filter(Boolean).join(" ");
}
