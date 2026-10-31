import type { RecipesAllResponse } from "../types/RecipesAllResponse";
import type { ExtractionRecipeResponse } from "../types/ExtractionRecipeResponse";
import type { ItemStack } from "../types/ItemStack";
import { resolveDropOutcomes } from "./dropEstimates";

export type ItemType = "Item" | "Cargo";
export type ResourceEffort = Record<string, number>;
export type InventoryQuantities = Record<ItemType, Record<number, number>>;
export const itemKey = (type: ItemType, id: number) => `${type}:${id}`;

export type CraftingIngredient = Readonly<Pick<ItemStack, "item_id" | "item_type" | "quantity">>;
export type GraphRecipe = Readonly<{
  id: number;
  name: string;
  actions: number;
  ingredients: readonly CraftingIngredient[];
  outputs: readonly CraftingIngredient[];
  kind: "craft" | "recraft" | "scrap";
}>;
export type RecipeOutput = Readonly<{
  key: string;
  recipeId: number;
  quantity: number;
  expectedQuantity: number;
  probability: number | null;
  fixedQuantity: number | null;
  minQuantity: number;
  recrafting: boolean;
}>;
export type GraphItem = Readonly<{
  id: number;
  type: ItemType;
  recipes: readonly RecipeOutput[];
  craftingRecipes: readonly RecipeOutput[];
  operations: readonly number[];
  gatherOnly: boolean;
  probabilistic: boolean;
  rank: number;
  defaultRecipe: string | null;
}>;
type BuildingGraphItem = {
  -readonly [Key in keyof GraphItem]: Key extends "recipes" | "craftingRecipes"
    ? RecipeOutput[]
    : Key extends "operations"
      ? number[]
      : GraphItem[Key];
};
export type CraftingGraph = {
  items: ReadonlyMap<string, GraphItem>;
  recipes: ReadonlyMap<number, GraphRecipe>;
};

// One recipe record and ingredient list per recipe, one output edge per item /
// recipe / yield. The graph never contains quantities or ancestor-specific trees.
export function buildCraftingGraph(data: RecipesAllResponse): CraftingGraph {
  const items = new Map<string, BuildingGraphItem>();
  const recipes = new Map<number, GraphRecipe>();
  function getItem(type: ItemType, id: number) {
    const key = itemKey(type, id);
    let item = items.get(key);
    if (!item) {
      const name = type === "Item" ? data.item_desc[id]?.name : data.cargo_desc[id]?.name;
      item = {
        id,
        type,
        recipes: [],
        craftingRecipes: [],
        operations: [],
        rank: Infinity,
        defaultRecipe: null,
        gatherOnly:
          type === "Item" && !!(name?.endsWith(" Animal Hair") || name?.endsWith(" Amber Resin")),
        probabilistic:
          type === "Item" &&
          (data.item_list_desc[data.item_desc[id]?.item_list_id ?? 0]?.possibilities.length ?? 0) >
            1,
      };
      items.set(key, item);
    }
    return item;
  }

  for (const source of Object.values(data.recipes)) {
    if (!source || source.name.includes("Package")) continue;
    const ingredients = source.consumed_item_stacks.filter((ingredient) => {
      const desc =
        ingredient.item_type === "Item"
          ? data.item_desc[ingredient.item_id]
          : data.cargo_desc[ingredient.item_id];
      return ingredient.quantity > 0 && !desc?.name.includes("Package");
    });
    // Filtering a package-only unpack recipe must not turn it into a free
    // production route. Genuine recipes with no inputs remain supported.
    if (source.consumed_item_stacks.length && !ingredients.length) continue;
    const recipe: GraphRecipe = {
      id: source.id,
      name: source.name,
      actions: source.actions_required,
      ingredients,
      outputs: source.crafted_item_stacks,
      kind: /^scrap\b/i.test(source.name)
        ? "scrap"
        : /^recraft\b/i.test(source.name)
          ? "recraft"
          : "craft",
    };
    recipes.set(recipe.id, recipe);
    const outcomes = new Map(
      resolveDropOutcomes(recipe.outputs, data).outcomes.map((output) => [
        itemKey(output.type, output.id),
        output,
      ]),
    );
    for (const ingredient of ingredients) getItem(ingredient.item_type, ingredient.item_id);
    // The first consumed stack is the existing item being operated on, not a
    // scrap/solvent catalyst. Index operations by that input, not their outputs.
    if (recipe.kind !== "craft" && ingredients[0]) {
      getItem(ingredients[0].item_type, ingredients[0].item_id).operations.push(recipe.id);
    }

    function addOutput(stack: CraftingIngredient, lists: Set<number>) {
      if (!(stack.quantity > 0)) return;
      const item = getItem(stack.item_type, stack.item_id);
      const key = `${recipe.id}:${stack.quantity}`;
      if (!item.recipes.some((output) => output.recipeId === recipe.id)) {
        const outcome = outcomes.get(itemKey(stack.item_type, stack.item_id));
        const expectedQuantity = outcome?.expectedQuantity ?? stack.quantity;
        const edge: RecipeOutput = {
          key,
          recipeId: recipe.id,
          quantity: stack.quantity,
          expectedQuantity,
          probability: outcome ? outcome.probability : 1,
          fixedQuantity: outcome
            ? outcome.minQuantity === outcome.maxQuantity
              ? outcome.minQuantity
              : null
            : stack.quantity,
          minQuantity: outcome?.minQuantity ?? stack.quantity,
          recrafting:
            /recraft/i.test(recipe.name) ||
            ingredients.some(
              (input) => input.item_type === stack.item_type && input.item_id === stack.item_id,
            ),
        };
        item.recipes.push(edge);
        if (edge.probability !== 1 || edge.fixedQuantity === null) item.probabilistic = true;
        if (recipe.kind === "craft" && !edge.recrafting) item.craftingRecipes.push(edge);
      }
      const listId = stack.item_type === "Item" ? data.item_desc[stack.item_id]?.item_list_id : 0;
      if (!listId || lists.has(listId)) return;
      const list = data.item_list_desc[listId];
      if (!list) return;
      lists.add(listId);
      for (const possibility of list.possibilities) {
        if (!(possibility.probability > 0)) continue;
        for (const output of possibility.items) addOutput(output, lists);
      }
      lists.delete(listId);
    }
    for (const output of source.crafted_item_stacks) addOutput(output, new Set());
  }

  // Relax ranks through the AND/OR graph, not through all possible paths. A
  // finite rank identifies a productive route; self-recrafting cannot create one.
  const dependents = new Map<
    BuildingGraphItem,
    { output: BuildingGraphItem; inputs: BuildingGraphItem[] }[]
  >();
  const queue: BuildingGraphItem[] = [];
  function update(item: BuildingGraphItem, rank: number) {
    if (rank >= item.rank) return;
    item.rank = rank;
    queue.push(item);
  }
  for (const item of items.values()) {
    if (!item.craftingRecipes.length || item.gatherOnly) update(item, 0);
    for (const output of item.craftingRecipes) {
      const recipe = recipes.get(output.recipeId)!;
      const inputs = [
        ...new Set(recipe.ingredients.map((input) => getItem(input.item_type, input.item_id))),
      ];
      const dependent = { output: item, inputs };
      if (!inputs.length) update(item, 1);
      for (const input of inputs) {
        const entries = dependents.get(input) ?? [];
        entries.push(dependent);
        dependents.set(input, entries);
      }
    }
  }
  for (let i = 0; i < queue.length; i++) {
    for (const { output, inputs } of dependents.get(queue[i]) ?? []) {
      update(output, 1 + Math.max(0, ...inputs.map((input) => input.rank)));
    }
  }
  // Among equally shallow productive routes, prefer the lower expected crafting
  // action cost, including prerequisite rarity odds. Rank order keeps this DAG-
  // based; it never explores alternative trees or recrafting cycles.
  const unitCosts = new Map<BuildingGraphItem, number>();
  for (const item of [...items.values()].sort((a, b) => a.rank - b.rank)) {
    if (!item.craftingRecipes.length || item.gatherOnly) {
      unitCosts.set(item, 0);
      continue;
    }
    let bestRank = Infinity;
    let bestCost = Infinity;
    for (const output of item.craftingRecipes) {
      const recipe = recipes.get(output.recipeId)!;
      const rank =
        1 +
        Math.max(
          0,
          ...recipe.ingredients.map(
            (input) => items.get(itemKey(input.item_type, input.item_id))!.rank,
          ),
        );
      if (!Number.isFinite(rank)) continue;
      const cost =
        (recipe.actions +
          recipe.ingredients.reduce(
            (sum, input) =>
              sum +
              input.quantity *
                (unitCosts.get(items.get(itemKey(input.item_type, input.item_id))!) ?? Infinity),
            0,
          )) /
        output.expectedQuantity;
      if (rank < bestRank || (rank === bestRank && cost < bestCost)) {
        item.defaultRecipe = output.key;
        bestRank = rank;
        bestCost = cost;
      }
    }
    unitCosts.set(item, bestCost);
  }
  return { items, recipes };
}

export function buildExtractionEffort(
  recipes: readonly ExtractionRecipeResponse[],
  data: RecipesAllResponse | undefined,
): ResourceEffort {
  const best: ResourceEffort = {};
  for (const recipe of recipes) {
    const expected: ResourceEffort = {};
    function add(stack: CraftingIngredient, probability: number) {
      const value = probability * stack.quantity;
      if (!(value > 0)) return;
      const key = itemKey(stack.item_type, stack.item_id);
      expected[key] = (expected[key] || 0) + value;
    }
    for (const extracted of recipe.extracted_item_stacks) {
      const stack = extracted.item_stack;
      if (!stack || !(extracted.probability > 0)) continue;
      const listId = stack.item_type === "Item" ? data?.item_desc[stack.item_id]?.item_list_id : 0;
      const list =
        stack.item_type === "Item" ? data?.item_list_desc[listId || stack.item_id] : undefined;
      if (list) {
        for (const possibility of list.possibilities) {
          for (const inner of possibility.items)
            add(inner, extracted.probability * stack.quantity * possibility.probability);
        }
      } else add(stack, extracted.probability);
    }
    for (const [key, yieldAmount] of Object.entries(expected)) {
      const attempts = 1 / yieldAmount;
      if (best[key] === undefined || attempts < best[key]) best[key] = attempts;
    }
  }
  return best;
}

// Only overrides are retained. The path identifies an occurrence, not a cached
// subtree; changing an ancestor discards every override below that branch.
export type RecipeSelections = ReadonlyMap<string, string | null>;
export function changeCraftingSelection(
  selections: RecipeSelections,
  path: string,
  recipe: string | null,
): RecipeSelections {
  if (selections.has(path) && selections.get(path) === recipe) return selections;
  const next = new Map(selections);
  for (const key of next.keys()) if (key.startsWith(`${path}/`)) next.delete(key);
  next.set(path, recipe);
  return next;
}

export type CraftingPlanNode = {
  path: string;
  id: number;
  type: ItemType;
  requested: number;
  owned: number;
  quantity: number;
  recipeKey: string | null;
  recipeId: number | null;
  recipeQuantity: number;
  probability: number | null;
  expectedYield: number | null;
  variableYield: boolean;
  attempts: number;
  expectedDemand: boolean;
  averageEstimate: boolean;
  recrafting: boolean;
  status: "craft" | "gather" | "inventory" | "requires-existing";
  actions: number;
  totalActions: number;
  children: CraftingPlanNode[];
};
export type CraftingPlan = {
  root: CraftingPlanNode;
  shoppingList: InventoryQuantities;
  nodeCount: number;
  probabilityAdjusted: boolean;
};

export function estimateCraftingAttempts(
  output: RecipeOutput,
  quantity: number,
  expectedDemand = false,
): number {
  if (!(quantity > 0)) return 0;
  if (
    !expectedDemand &&
    output.probability !== null &&
    output.probability > 0 &&
    quantity <= output.minQuantity
  )
    return 1 / output.probability;
  if (output.probability !== null && output.probability > 0 && output.fixedQuantity !== null) {
    const successes = expectedDemand
      ? quantity / output.fixedQuantity
      : Math.ceil(quantity / output.fixedQuantity);
    return successes / output.probability;
  }
  return quantity / output.expectedQuantity;
}

export function buildCraftingPlan(
  graph: CraftingGraph,
  type: ItemType,
  id: number,
  quantity: number,
  options: {
    selections?: RecipeSelections;
    inventory?: InventoryQuantities;
    extraction?: ResourceEffort;
  } = {},
): CraftingPlan {
  const remaining = new Map<string, number>();
  for (const type of ["Item", "Cargo"] as const) {
    for (const [id, amount] of Object.entries(options.inventory?.[type] ?? {})) {
      if (Number.isFinite(amount) && amount > 0) remaining.set(itemKey(type, Number(id)), amount);
    }
  }
  const shoppingList: InventoryQuantities = { Item: {}, Cargo: {} };
  const ancestors = new Set<string>();
  let nodeCount = 0;

  function visit(
    type: ItemType,
    id: number,
    requested: number,
    path: string,
    expectedDemand = false,
  ): CraftingPlanNode {
    nodeCount++;
    const key = itemKey(type, id);
    const item = graph.items.get(key);
    const chosen = options.selections?.has(path)
      ? options.selections.get(path)
      : item?.defaultRecipe;
    const output =
      item?.recipes.find((output) => output.key === chosen) ??
      (chosen === null
        ? undefined
        : item?.recipes.find((output) => output.key === item.defaultRecipe));
    const cycle = ancestors.has(key);
    // An explicit recraft consumes the existing equipment as an ingredient,
    // rather than counting that equipment as an already-finished output.
    const forceRecraft =
      !cycle && !!output?.recrafting && options.selections?.has(path) && chosen === output.key;
    const owned = forceRecraft ? 0 : Math.min(requested, remaining.get(key) ?? 0);
    if (owned > 0) remaining.set(key, (remaining.get(key) ?? 0) - owned);
    const needed = requested - owned;
    const node: CraftingPlanNode = {
      path,
      id,
      type,
      requested,
      owned,
      quantity: needed,
      recipeKey: output?.key ?? null,
      recipeId: output?.recipeId ?? null,
      recipeQuantity: output?.quantity ?? 1,
      probability: output?.probability ?? null,
      expectedYield: output?.expectedQuantity ?? null,
      variableYield: output?.fixedQuantity === null,
      attempts: 0,
      expectedDemand,
      averageEstimate: expectedDemand,
      recrafting: output?.recrafting ?? false,
      status: "gather",
      actions: 0,
      totalActions: 0,
      children: [],
    };
    if (needed <= 0) {
      node.status = "inventory";
      return node;
    }

    if (cycle || !output) {
      node.recipeId = null;
      node.recipeKey = null;
      const effort = options.extraction?.[key];
      const suppliedDirectly =
        item?.gatherOnly || (options.selections?.has(path) && chosen === null);
      node.status =
        effort === undefined && (cycle || (!!item?.craftingRecipes.length && !suppliedDirectly))
          ? "requires-existing"
          : "gather";
      node.actions =
        effort === undefined ? 0 : expectedDemand ? effort * needed : Math.ceil(effort * needed);
      node.totalActions = node.actions;
      shoppingList[type][id] = (shoppingList[type][id] || 0) + needed;
      return node;
    }

    const recipe = graph.recipes.get(output.recipeId)!;
    node.status = "craft";
    const averageDemand =
      expectedDemand || output.probability !== 1 || output.fixedQuantity === null;
    node.averageEstimate = averageDemand;
    node.attempts = estimateCraftingAttempts(output, needed, expectedDemand);
    node.actions = recipe.actions * node.attempts;
    node.totalActions = node.actions;
    ancestors.add(key);
    for (let i = 0; i < recipe.ingredients.length; i++) {
      const input = recipe.ingredients[i];
      const child = visit(
        input.item_type,
        input.item_id,
        averageDemand
          ? node.attempts * input.quantity
          : Math.ceil((needed * input.quantity) / output.expectedQuantity),
        `${path}/${output.key}/${i}`,
        averageDemand,
      );
      node.children.push(child);
      node.totalActions += child.totalActions;
      node.averageEstimate ||= child.averageEstimate;
    }
    ancestors.delete(key);
    return node;
  }
  const amount = Number.isFinite(quantity) ? Math.max(1, Math.ceil(quantity)) : 1;
  const root = visit(type, id, amount, itemKey(type, id));
  // Fractional expected demand propagates through the whole selected path.
  // Round only the final aggregated shopping totals, not every rarity stage.
  for (const type of ["Item", "Cargo"] as const) {
    for (const id of Object.keys(shoppingList[type])) {
      const value = shoppingList[type][Number(id)];
      const rounded = Math.round(value);
      shoppingList[type][Number(id)] =
        Math.abs(value - rounded) <= Number.EPSILON * Math.max(1, value) * 16
          ? rounded
          : Math.ceil(value);
    }
  }
  return { root, shoppingList, nodeCount, probabilityAdjusted: root.averageEstimate };
}
