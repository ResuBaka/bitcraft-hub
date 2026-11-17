type Integer = number | bigint;
export type HexCoinEstimate = {
  estimated_remaining_coins: Integer | null;
  xp_per_coin: Integer | null;
  claim_xp_remainder: Integer | null;
  assumed_xp_per_hit: Integer | null;
};

function integer(value: Integer | null | undefined): bigint | null {
  if (value == null || (typeof value === "number" && !Number.isSafeInteger(value))) return null;
  const result = BigInt(value);
  return result >= 0n ? result : null;
}

export function remainingHexCoins(
  craft: { kind: string; progress: number | null; progress_total: Integer | null; hex_coin_estimate?: HexCoinEstimate | null },
  liveRemainder?: Integer | null,
): bigint | null {
  const estimate = craft.hex_coin_estimate;
  if (!estimate) return null;
  if (craft.kind === "Passive") return integer(estimate.estimated_remaining_coins);
  const progress = integer(craft.progress);
  const total = integer(craft.progress_total);
  if (progress !== null && total !== null && progress >= total) return 0n;
  const rate = integer(estimate.xp_per_coin);
  const xp = integer(estimate.assumed_xp_per_hit);
  const remainder = integer(liveRemainder ?? estimate.claim_xp_remainder);
  if (progress !== null && total !== null && rate !== null && rate > 0n && xp !== null && remainder !== null) {
    return ((total - progress) * xp + remainder) / rate;
  }
  return integer(estimate.estimated_remaining_coins);
}
