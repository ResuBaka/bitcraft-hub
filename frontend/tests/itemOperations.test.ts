import { describe, expect, test } from "bun:test";
import { buildCraftingGraph, buildCraftingPlan } from "../app/utils/craftingPlan";
import { buildItemOperation, getItemOperationRecipes } from "../app/utils/itemOperations";
import type { CraftingRecipe } from "../app/types/CraftingRecipe";
import type { RecipesAllResponse } from "../app/types/RecipesAllResponse";

function recipe(
  id: number,
  name: string,
  input: number,
  output: number,
  quantity = 1,
): CraftingRecipe {
  return {
    id,
    name,
    actions_required: 10,
    consumed_item_stacks: [{ item_id: input, item_type: "Item", quantity: 1 }],
    crafted_item_stacks: [{ item_id: output, item_type: "Item", quantity }],
  } as CraftingRecipe;
}
function data(...recipes: CraftingRecipe[]): RecipesAllResponse {
  return {
    recipes: Object.fromEntries(recipes.map((recipe) => [recipe.id, recipe])),
    item_desc: {},
    cargo_desc: {},
    item_list_desc: {},
  };
}
const emptyInventory = { Item: {}, Cargo: {} };

describe("special operation classification", () => {
  test("special output pages list scrap sources without duplicating operations", () => {
    const graph = buildCraftingGraph(
      data(recipe(1, "Scrap {1}", 10, 20), recipe(2, "Recraft {1}", 30, 30)),
    );
    expect(getItemOperationRecipes(graph, "Item", 20).map((recipe) => recipe.id)).toEqual([1]);
    expect(getItemOperationRecipes(graph, "Item", 10).map((recipe) => recipe.id)).toEqual([1]);
    expect(getItemOperationRecipes(graph, "Item", 30).map((recipe) => recipe.id)).toEqual([2]);
  });
  test("keeps recraft and scrap out of normal craft choices and defaults", () => {
    const graph = buildCraftingGraph(
      data(
        recipe(1, "Craft {0}", 20, 10),
        recipe(2, "Scrap {1}", 30, 10, 5),
        recipe(3, "Recraft {1}", 10, 10),
      ),
    );
    const item = graph.items.get("Item:10")!;
    expect(item.recipes).toHaveLength(3);
    expect(item.craftingRecipes.map((output) => output.recipeId)).toEqual([1]);
    expect(item.defaultRecipe).toBe("1:1");
    expect(item.operations).toEqual([3]);
    expect(graph.items.get("Item:30")!.operations).toEqual([2]);
    expect(buildCraftingPlan(graph, "Item", 10, 1).shoppingList.Item).toEqual({ 20: 1 });
  });

  test("scrap-only outputs are supplied materials, never a plan to manufacture and destroy equipment", () => {
    const graph = buildCraftingGraph(
      data(recipe(1, "Scrap {1}", 10, 20, 15), recipe(2, "Craft {0}", 30, 10)),
    );
    const plan = buildCraftingPlan(graph, "Item", 20, 5);
    expect(plan.nodeCount).toBe(1);
    expect(plan.root.status).toBe("gather");
    expect(plan.shoppingList.Item).toEqual({ 20: 5 });
  });

  test("indexes existing equipment, not scrap and solvent catalysts", () => {
    const source = recipe(1, "Recraft {1}", 10, 40);
    source.consumed_item_stacks.push({ item_id: 20, item_type: "Item", quantity: 15 } as any);
    const graph = buildCraftingGraph(data(source));
    expect(graph.items.get("Item:10")!.operations).toEqual([1]);
    expect(graph.items.get("Item:20")!.operations).toEqual([]);
    expect(graph.items.get("Item:40")!.operations).toEqual([]);
  });

  test("does not misclassify normal recipes that return a catalyst as recrafting", () => {
    const source = recipe(1, "Enrich {0}", 10, 20);
    source.crafted_item_stacks.push({ item_id: 10, item_type: "Item", quantity: 1 } as any);
    const graph = buildCraftingGraph(data(source));
    expect(graph.recipes.get(1)!.kind).toBe("craft");
    expect(graph.items.get("Item:20")!.craftingRecipes).toHaveLength(1);
    expect(graph.items.get("Item:10")!.craftingRecipes).toHaveLength(0);
    expect(graph.items.get("Item:10")!.operations).toHaveLength(0);
  });
});

describe("existing item operation calculator", () => {
  test("scrap consumes existing equipment and reports output and direct action counts", () => {
    const input = data(recipe(1, "Scrap {1}", 10, 20, 15), recipe(2, "Craft {0}", 30, 10));
    const graph = buildCraftingGraph(input);
    const inventory = { Item: { 10: 2 }, Cargo: {} };
    const preview = buildItemOperation(graph, input, 1, 3, inventory)!;
    expect(preview.runs).toBe(3);
    expect(preview.actions).toBe(30);
    expect(preview.requirements).toEqual([
      { id: 10, type: "Item", quantity: 3, owned: 2, missing: 1, existing: true },
    ]);
    expect(preview.outputs).toMatchObject([{ id: 20, type: "Item", quantity: 45, probability: 1 }]);
    expect(preview.requirements.some((input) => input.id === 30)).toBe(false);
    expect(inventory.Item[10]).toBe(2);
  });

  test("recraft inputs do not recursively craft their own ingredients", () => {
    const source = recipe(1, "Recraft {1}", 10, 40);
    source.consumed_item_stacks.push({ item_id: 20, item_type: "Item", quantity: 2 } as any);
    const input = data(source, recipe(2, "Craft {0}", 30, 20));
    const graph = buildCraftingGraph(input);
    const preview = buildItemOperation(graph, input, 1, 1, {
      Item: { 10: 1, 30: 100 },
      Cargo: {},
    })!;
    expect(preview.requirements.map((input) => input.id)).toEqual([10, 20]);
    expect(preview.requirements[1].missing).toBe(2);
    expect(preview.actions).toBe(10);
  });

  test("shows expected quality outcomes rather than a guaranteed result", () => {
    const input = data(recipe(1, "Recraft {1}", 10, 40));
    input.item_desc[40] = { item_list_id: 1 } as any;
    input.item_list_desc[1] = {
      possibilities: [
        { probability: 0.925, items: [{ item_id: 10, item_type: "Item", quantity: 1 }] },
        { probability: 0.075, items: [{ item_id: 50, item_type: "Item", quantity: 1 }] },
      ],
    } as any;
    const preview = buildItemOperation(buildCraftingGraph(input), input, 1, 2, emptyInventory)!;
    expect(preview.probabilistic).toBe(true);
    expect(preview.outputs).toMatchObject([
      { id: 10, type: "Item", quantity: 1.85 },
      { id: 50, type: "Item", quantity: 0.15 },
    ]);
    expect(preview.outputs[0].probability).toBeCloseTo(0.925);
    expect(preview.outputs[1].probability).toBeCloseTo(0.075);
  });

  test("combines duplicate input stacks before allocating inventory", () => {
    const source = recipe(1, "Scrap {1}", 10, 20);
    source.consumed_item_stacks.push({ item_id: 10, item_type: "Item", quantity: 2 } as any);
    const input = data(source);
    const preview = buildItemOperation(buildCraftingGraph(input), input, 1, 1, {
      Item: { 10: 2 },
      Cargo: {},
    })!;
    expect(preview.requirements).toEqual([
      { id: 10, type: "Item", quantity: 3, owned: 2, missing: 1, existing: true },
    ]);
  });

  test("ignores normal recipes and sanitizes the run count", () => {
    const input = data(recipe(1, "Craft {0}", 10, 20), recipe(2, "Scrap {1}", 30, 40));
    const graph = buildCraftingGraph(input);
    expect(buildItemOperation(graph, input, 1, 1, emptyInventory)).toBeUndefined();
    expect(buildItemOperation(graph, input, 99, 1, emptyInventory)).toBeUndefined();
    expect(buildItemOperation(graph, input, 2, NaN, emptyInventory)!.runs).toBe(1);
  });
});
