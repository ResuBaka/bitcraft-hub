import { formatCountdown } from "./countdown";

export function meterValue(
  value: number | null | undefined,
  max: number | null | undefined,
): number | null {
  if (value == null || max == null || !Number.isFinite(value) || !Number.isFinite(max) || max <= 0)
    return null;
  return Math.min(max, Math.max(0, value));
}

type TimedBuff = {
  is_food_associated: boolean | null;
  remaining_seconds: number | bigint | null;
  duration: number;
  paused: boolean;
};

export function activeBuffsWithTime<T extends TimedBuff>(buffs: T[], elapsedSeconds: number) {
  return buffs.flatMap((buff) => {
    const remaining =
      buff.remaining_seconds === null
        ? null
        : Math.max(0, Number(buff.remaining_seconds) - (buff.paused ? 0 : elapsedSeconds));
    return remaining === 0 ? [] : [{ ...buff, remaining }];
  });
}

export function summarizeFoodBuffs<T extends TimedBuff & { remaining: number | null }>(buffs: T[]) {
  const active = buffs.filter((buff) => buff.is_food_associated === true);
  const ticking = active
    .filter((buff) => buff.remaining !== null && !buff.paused)
    .sort((a, b) => a.remaining! - b.remaining!);
  const paused = active
    .filter((buff) => buff.remaining !== null && buff.paused)
    .sort((a, b) => a.remaining! - b.remaining!);
  return { count: active.length, next: ticking[0] ?? paused[0] ?? active[0] ?? null };
}
export function formatFoodTimer(seconds: number): string {
  return formatCountdown(seconds * 1000, "minute");
}
