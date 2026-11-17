import { expect, test } from "bun:test";
import { remainingHexCoins } from "../app/utils/hexCoinEstimate";

test("recalculates the estimate with live progress and the shared remainder", () => {
  const craft = { kind: "Active", progress: 2, progress_total: 12n, hex_coin_estimate: { estimated_remaining_coins: 3n, xp_per_coin: 6, claim_xp_remainder: 3, assumed_xp_per_hit: 2n } };
  expect(remainingHexCoins(craft)).toBe(3n);
  expect(remainingHexCoins({ ...craft, progress: 8 }, 5)).toBe(2n);
  expect(remainingHexCoins({ ...craft, progress: 12 }, 5)).toBe(0n);
});

test("preserves unknown and non-minting estimates", () => {
  const craft = { kind: "Active", progress: 1, progress_total: 10, hex_coin_estimate: { estimated_remaining_coins: null, xp_per_coin: null, claim_xp_remainder: null, assumed_xp_per_hit: null } };
  expect(remainingHexCoins(craft)).toBeNull();
  expect(remainingHexCoins({ ...craft, hex_coin_estimate: { ...craft.hex_coin_estimate, estimated_remaining_coins: 0 } })).toBe(0n);
  expect(remainingHexCoins({ ...craft, kind: "Passive", hex_coin_estimate: { ...craft.hex_coin_estimate, estimated_remaining_coins: 0 } })).toBe(0n);
  expect(remainingHexCoins({ ...craft, hex_coin_estimate: null })).toBeNull();
});
