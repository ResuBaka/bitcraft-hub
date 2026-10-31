import type { RecipesAllResponse } from "../types/RecipesAllResponse";
import type { CraftingIngredient, ItemType } from "./craftingPlan";
// Keep this resolver independent of the planner (which also consumes its odds).
const itemKey = (type: ItemType, id: number) => `${type}:${id}`;

export type DropOutcome = {
  id: number;
  type: ItemType;
  expectedQuantity: number;
  probability: number | null;
  minQuantity: number;
  maxQuantity: number;
};

// Marginals are sufficient: do not enumerate the Cartesian product of outcomes.
// Possibilities in a list are exclusive; separate stacks / list draws are independent.
export function resolveDropOutcomes(
  stacks: readonly CraftingIngredient[],
  data: RecipesAllResponse,
) {
  type Outcomes = Map<string, DropOutcome>;
  const lists = new Set<number>();
  let probabilistic = false;
  function combine(target: Outcomes, source: Outcomes) {
    for (const [key, outcome] of source) {
      const existing = target.get(key);
      if (!existing) {
        target.set(key, { ...outcome });
        continue;
      }
      existing.expectedQuantity += outcome.expectedQuantity;
      const a = existing.probability;
      const b = outcome.probability;
      existing.minQuantity =
        a === 1 && b === 1
          ? existing.minQuantity + outcome.minQuantity
          : a === 1
            ? existing.minQuantity
            : b === 1
              ? outcome.minQuantity
              : Math.min(existing.minQuantity, outcome.minQuantity);
      existing.maxQuantity += outcome.maxQuantity;
      existing.probability =
        a === 1 || b === 1 ? 1 : a === null || b === null ? null : a + b - a * b;
    }
  }
  function stackOutcomes(stack: CraftingIngredient): Outcomes {
    if (!(stack.quantity > 0)) return new Map();
    const listId = stack.item_type === "Item" ? data.item_desc[stack.item_id]?.item_list_id : 0;
    const list = listId && !lists.has(listId) ? data.item_list_desc[listId] : undefined;
    if (!list)
      return new Map([
        [
          itemKey(stack.item_type, stack.item_id),
          {
            id: stack.item_id,
            type: stack.item_type,
            expectedQuantity: stack.quantity,
            probability: listId && lists.has(listId) ? null : 1,
            minQuantity: stack.quantity,
            maxQuantity: stack.quantity,
          },
        ],
      ]);

    lists.add(listId!);
    const result: Outcomes = new Map();
    let totalProbability = 0;
    for (const possibility of list.possibilities) {
      const probability = possibility.probability;
      if (!(probability > 0) || !Number.isFinite(probability)) continue;
      totalProbability += probability;
      if (probability !== 1) probabilistic = true;
      const branch: Outcomes = new Map();
      for (const inner of possibility.items) combine(branch, stackOutcomes(inner));
      for (const [key, outcome] of branch) {
        const existing = result.get(key) ?? {
          ...outcome,
          expectedQuantity: 0,
          probability: 0,
          minQuantity: Infinity,
          maxQuantity: 0,
        };
        existing.expectedQuantity += probability * outcome.expectedQuantity;
        existing.minQuantity = Math.min(existing.minQuantity, outcome.minQuantity);
        existing.maxQuantity = Math.max(existing.maxQuantity, outcome.maxQuantity);
        existing.probability =
          existing.probability === null || outcome.probability === null
            ? null
            : existing.probability + probability * outcome.probability;
        result.set(key, existing);
      }
    }
    lists.delete(listId!);
    // MessagePack exposes the game's float32 values (e.g. .925 + .075 can
    // arrive as 1.0000000149). Do not mistake that rounding for overlapping odds.
    const invalidTotal = totalProbability > 1 + 1e-6;
    if (invalidTotal) probabilistic = true;
    for (const outcome of result.values()) {
      outcome.expectedQuantity *= stack.quantity;
      outcome.maxQuantity *= stack.quantity;
      // Fractional draw counts or overlapping / malformed lists do not have
      // enough information for a reliable at-least-one probability.
      if (!Number.isInteger(stack.quantity) || invalidTotal) outcome.probability = null;
      else if (outcome.probability !== null) {
        const probability = Math.min(
          1,
          Math.max(0, outcome.probability / Math.max(1, totalProbability)),
        );
        if (probability === 1) outcome.minQuantity *= stack.quantity;
        outcome.probability =
          stack.quantity === 1 || probability === 1
            ? probability
            : -Math.expm1(stack.quantity * Math.log1p(-probability));
      }
    }
    return result;
  }
  const outcomes: Outcomes = new Map();
  for (const stack of stacks) combine(outcomes, stackOutcomes(stack));
  return { outcomes: [...outcomes.values()], probabilistic };
}

export function chanceAfterAttempts(probability: number, attempts: number): number {
  if (attempts <= 0 || probability <= 0) return 0;
  if (probability >= 1) return 1;
  return -Math.expm1(Math.floor(attempts) * Math.log1p(-probability));
}

export function attemptsForChance(probability: number, confidence: number): number | null {
  if (!(probability > 0 && probability <= 1) || !(confidence > 0 && confidence <= 1)) return null;
  if (probability === 1) return 1;
  if (confidence === 1) return null;
  const value = Math.log1p(-confidence) / Math.log1p(-probability);
  const rounded = Math.round(value);
  // Avoid rounding an exact boundary (e.g. 90% odds, 99% confidence) up a run.
  const runs =
    Math.abs(value - rounded) <= Number.EPSILON * Math.max(1, value) * 8
      ? rounded
      : Math.ceil(value);
  return Number.isSafeInteger(runs) && runs > 0 ? runs : null;
}

export function estimateDrop(probability: number | null, attempts: number) {
  if (probability === null || !Number.isFinite(probability) || probability < 0 || probability > 1)
    return null;
  return {
    probability,
    averageAttempts: probability > 0 ? 1 / probability : Infinity,
    chanceAfterAttempts: chanceAfterAttempts(probability, attempts),
    attempts50: attemptsForChance(probability, 0.5),
    attempts90: attemptsForChance(probability, 0.9),
    attempts95: attemptsForChance(probability, 0.95),
    attempts99: attemptsForChance(probability, 0.99),
  };
}
