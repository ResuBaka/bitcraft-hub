import { describe, expect, test } from "bun:test";
import {
  attemptsForChance,
  chanceAfterAttempts,
  estimateDrop,
  resolveDropOutcomes,
} from "../app/utils/dropEstimates";
import type { RecipesAllResponse } from "../app/types/RecipesAllResponse";
import type { CraftingIngredient } from "../app/utils/craftingPlan";

const stack = (id: number, quantity = 1): CraftingIngredient => ({
  item_id: id,
  item_type: "Item",
  quantity,
});
function data(
  lists: [number, { probability: number; items: CraftingIngredient[] }[]][],
): RecipesAllResponse {
  return {
    recipes: {},
    cargo_desc: {},
    item_desc: Object.fromEntries(lists.map(([id]) => [id, { item_list_id: id }])) as any,
    item_list_desc: Object.fromEntries(
      lists.map(([id, possibilities]) => [id, { possibilities }]),
    ) as any,
  };
}

describe("attempt estimates", () => {
  test("7.5% drop has a 13.33-run mean but needs 39 runs for a 95% chance", () => {
    const estimate = estimateDrop(0.075, 2)!;
    expect(estimate.averageAttempts).toBeCloseTo(13.333333);
    expect(estimate.attempts50).toBe(9);
    expect(estimate.attempts90).toBe(30);
    expect(estimate.attempts95).toBe(39);
    expect(estimate.attempts99).toBe(60);
    expect(estimate.chanceAfterAttempts).toBeCloseTo(0.144375);
  });

  test("confidence counts are minimal and meet the requested confidence", () => {
    for (const probability of [0.000001, 0.01, 0.075, 0.5, 0.9, 0.9999]) {
      for (const confidence of [0.5, 0.9, 0.95, 0.99]) {
        const n = attemptsForChance(probability, confidence)!;
        expect(chanceAfterAttempts(probability, n) + 1e-12).toBeGreaterThanOrEqual(confidence);
        expect(chanceAfterAttempts(probability, n - 1)).toBeLessThan(confidence);
      }
    }
  });

  test("does not add an extra run at an exact floating-point boundary", () => {
    expect(attemptsForChance(0.9, 0.99)).toBe(2);
    expect(attemptsForChance(0.5, 0.75)).toBe(2);
    expect(attemptsForChance(1, 1)).toBe(1);
    expect(attemptsForChance(0.5, 1)).toBeNull();
  });

  test("handles certain and impossible drops without bogus confidence counts", () => {
    const certain = estimateDrop(1, 1)!;
    expect(certain.averageAttempts).toBe(1);
    expect(certain.attempts95).toBe(1);
    expect(certain.chanceAfterAttempts).toBe(1);
    const impossible = estimateDrop(0, 100)!;
    expect(impossible.averageAttempts).toBe(Infinity);
    expect(impossible.attempts95).toBeNull();
    expect(impossible.chanceAfterAttempts).toBe(0);
    expect(chanceAfterAttempts(1, 0)).toBe(0);
  });

  test("uses stable math for tiny probabilities and refuses unsafe run counts", () => {
    expect(chanceAfterAttempts(1e-10, 1e10)).toBeCloseTo(1 - Math.exp(-1), 8);
    expect(attemptsForChance(1e-20, 0.95)).toBeNull();
    for (const probability of [null, NaN, Infinity, -0.1, 1.1])
      expect(estimateDrop(probability, 1)).toBeNull();
  });
});

describe("per-attempt drop probabilities", () => {
  test("accepts float32 rounding from the real MessagePack outcome weights", () => {
    const input = data([
      [
        10,
        [
          { probability: Math.fround(0.925), items: [stack(20)] },
          { probability: Math.fround(0.075), items: [stack(30)] },
        ],
      ],
    ]);
    const { outcomes } = resolveDropOutcomes([stack(10)], input);
    expect(outcomes[0].probability).toBeCloseTo(0.925);
    expect(outcomes[1].probability).toBeCloseTo(0.075);
    expect(estimateDrop(outcomes[1].probability, 1)!.attempts95).toBe(39);
  });
  test("expected quantity is not confused with the chance of a drop", () => {
    const input = data([[10, [{ probability: 0.25, items: [stack(20, 4)] }]]]);
    const { outcomes } = resolveDropOutcomes([stack(10)], input);
    expect(outcomes[0].expectedQuantity).toBe(1);
    expect(outcomes[0].probability).toBeCloseTo(0.25);
    expect(estimateDrop(outcomes[0].probability, 1)!.averageAttempts).toBeCloseTo(4);
  });

  test("sums mutually exclusive outcomes for the same item", () => {
    const input = data([
      [
        10,
        [
          { probability: 0.25, items: [stack(20)] },
          { probability: 0.5, items: [stack(20, 3)] },
        ],
      ],
    ]);
    const { outcomes } = resolveDropOutcomes([stack(10)], input);
    expect(outcomes[0].expectedQuantity).toBe(1.75);
    expect(outcomes[0].probability).toBeCloseTo(0.75);
  });

  test("duplicate stacks in the same outcome do not double its probability", () => {
    const input = data([[10, [{ probability: 0.5, items: [stack(20), stack(20)] }]]]);
    const { outcomes } = resolveDropOutcomes([stack(10)], input);
    expect(outcomes[0].expectedQuantity).toBe(1);
    expect(outcomes[0].probability).toBeCloseTo(0.5);
  });

  test("combines independent output stacks by the chance of at least one", () => {
    const input = data([
      [10, [{ probability: 0.2, items: [stack(30)] }]],
      [20, [{ probability: 0.3, items: [stack(30)] }]],
    ]);
    const { outcomes } = resolveDropOutcomes([stack(10), stack(20)], input);
    expect(outcomes[0].expectedQuantity).toBeCloseTo(0.5);
    expect(outcomes[0].probability).toBeCloseTo(0.44);
  });

  test("supports nested probabilities and repeated list draws", () => {
    const input = data([
      [10, [{ probability: 0.5, items: [stack(20)] }]],
      [20, [{ probability: 0.2, items: [stack(30, 2)] }]],
    ]);
    const nested = resolveDropOutcomes([stack(10)], input).outcomes[0];
    expect(nested.expectedQuantity).toBeCloseTo(0.2);
    expect(nested.probability).toBeCloseTo(0.1);
    const repeated = resolveDropOutcomes([stack(20, 2)], input).outcomes[0];
    expect(repeated.expectedQuantity).toBeCloseTo(0.8);
    expect(repeated.probability).toBeCloseTo(0.36);
  });

  test("an item can be guaranteed even though its quantity is random", () => {
    const input = data([
      [
        10,
        [
          { probability: 0.5, items: [stack(20)] },
          { probability: 0.5, items: [stack(20, 3)] },
        ],
      ],
    ]);
    const result = resolveDropOutcomes([stack(10)], input);
    expect(result.probabilistic).toBe(true);
    expect(result.outcomes[0].expectedQuantity).toBe(2);
    expect(result.outcomes[0].probability).toBe(1);
  });

  test("does not invent odds for cyclic or overlapping list data", () => {
    const cycle = data([[10, [{ probability: 1, items: [stack(10)] }]]]);
    expect(resolveDropOutcomes([stack(10)], cycle).outcomes[0].probability).toBeNull();
    const overlap = data([
      [
        10,
        [
          { probability: 0.8, items: [stack(20)] },
          { probability: 0.8, items: [stack(20)] },
        ],
      ],
    ]);
    expect(resolveDropOutcomes([stack(10)], overlap).outcomes[0].probability).toBeNull();
  });
});
