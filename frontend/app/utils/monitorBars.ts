export function meterValue(
  value: number | null | undefined,
  max: number | null | undefined,
): number | null {
  if (value == null || max == null || !Number.isFinite(value) || !Number.isFinite(max) || max <= 0)
    return null;
  return Math.min(max, Math.max(0, value));
}

type FoodBuff = {
  is_food_associated: boolean | null;
  remaining_seconds: number | null;
  duration: number;
  paused: boolean;
};

export function summarizeFoodBuffs<T extends FoodBuff>(buffs: T[], elapsedSeconds: number) {
  const active = buffs.flatMap((buff) => {
    if (buff.is_food_associated !== true) return [];
    const remaining =
      buff.remaining_seconds === null
        ? null
        : Math.max(0, buff.remaining_seconds - (buff.paused ? 0 : elapsedSeconds));
    return remaining === 0 ? [] : [{ ...buff, remaining }];
  });
  const ticking = active
    .filter((buff) => buff.remaining !== null && !buff.paused)
    .sort((a, b) => a.remaining! - b.remaining!);
  const paused = active
    .filter((buff) => buff.remaining !== null && buff.paused)
    .sort((a, b) => a.remaining! - b.remaining!);
  return { count: active.length, next: ticking[0] ?? paused[0] ?? active[0] ?? null };
}
export function formatFoodTimer(seconds: number): string {
  const remaining = Math.max(0, Math.floor(seconds));
  return remaining >= 60 ? `${Math.floor(remaining / 60)}m` : `${remaining}s`;
}
