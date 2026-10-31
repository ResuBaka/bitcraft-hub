import type { RecipesAllResponse } from "../types/RecipesAllResponse";
import { itemKey } from "./craftingPlan";
import { resolveDropOutcomes } from "./dropEstimates";
import type { CraftingGraph, InventoryQuantities, ItemType } from "./craftingPlan";

export type OperationRequirement = {
  id: number;
  type: ItemType;
  quantity: number;
  owned: number;
  missing: number;
  existing: boolean;
};

// Include operations on this item and special ways to obtain it. For example,
// armor scrap's page lists the equipment that can be scrapped for that material.
export function getItemOperationRecipes(graph: CraftingGraph, type: ItemType, id: number) {
  const item = graph.items.get(itemKey(type, id));
  const ids = new Set(item?.operations ?? []);
  for (const output of item?.recipes ?? []) {
    if (graph.recipes.get(output.recipeId)?.kind !== "craft") ids.add(output.recipeId);
  }
  return [...ids]
    .map((id) => graph.recipes.get(id)!)
    .filter((recipe) => recipe && recipe.kind !== "craft");
}
export type ItemOperation = {
  recipeId: number;
  kind: "recraft" | "scrap";
  runs: number;
  actions: number;
  requirements: OperationRequirement[];
  outputs: { id: number; type: ItemType; quantity: number; probability: number | null }[];
  probabilistic: boolean;
};

// Special operations consume existing inputs. This intentionally does not call
// the crafting planner: never manufacture equipment just to scrap or reroll it.
export function buildItemOperation(
  graph: CraftingGraph,
  data: RecipesAllResponse,
  recipeId: number,
  requestedRuns: number,
  inventory: InventoryQuantities,
): ItemOperation | undefined {
  const recipe = graph.recipes.get(recipeId);
  if (!recipe || recipe.kind === "craft") return;
  const runs = Number.isFinite(requestedRuns) ? Math.max(1, Math.ceil(requestedRuns)) : 1;
  const requirements = new Map<string, OperationRequirement>();
  const first = recipe.ingredients[0];
  const existingKey = first ? itemKey(first.item_type, first.item_id) : undefined;
  for (const input of recipe.ingredients) {
    const key = itemKey(input.item_type, input.item_id);
    const entry = requirements.get(key) ?? {
      id: input.item_id,
      type: input.item_type,
      quantity: 0,
      owned: 0,
      missing: 0,
      existing: key === existingKey,
    };
    entry.quantity += input.quantity * runs;
    requirements.set(key, entry);
  }
  for (const input of requirements.values()) {
    const stock = inventory[input.type][input.id] ?? 0;
    input.owned = Number.isFinite(stock) ? Math.min(input.quantity, Math.max(0, stock)) : 0;
    input.missing = input.quantity - input.owned;
  }

  const { outcomes, probabilistic } = resolveDropOutcomes(recipe.outputs, data);
  return {
    recipeId,
    kind: recipe.kind,
    runs,
    actions: recipe.actions * runs,
    requirements: [...requirements.values()],
    outputs: outcomes.map((output) => ({
      id: output.id,
      type: output.type,
      quantity: output.expectedQuantity * runs,
      probability: output.probability,
    })),
    probabilistic,
  };
}
