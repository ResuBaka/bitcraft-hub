import { expect, test } from "bun:test";
import type { RecipesAllResponse } from "../app/types/RecipesAllResponse";
import {
  buildCraftingGraph,
  buildCraftingPlan,
  changeCraftingSelection,
} from "../app/utils/craftingPlan";
import type { RecipeSelections } from "../app/utils/craftingPlan";

// CRAFTING_TEST_API=http://localhost:8000 bun test tests/craftingPlan.integration.test.ts
const api = process.env.CRAFTING_TEST_API;
test.skipIf(!api)(
  "Umbracite Plated Boots keeps one selected plan through repeated recipe switches",
  async () => {
    const response = await fetch(`${api!.replace(/\/$/, "")}/recipes/get_all`, {
      headers: { Accept: "application/json" },
    });
    expect(response.ok).toBe(true);
    const data = (await response.json()) as RecipesAllResponse;
    expect(data.item_desc[17840106].name).toBe("Umbracite Plated Boots");
    const start = performance.now();
    const graph = buildCraftingGraph(data);
    const initial = buildCraftingPlan(graph, "Item", 17840106, 1);
    expect(initial.nodeCount).toBeLessThan(500);
    expect(Object.keys(initial.shoppingList.Item).length).toBeGreaterThan(0);
    const outputs = graph.items.get("Item:17840106")!.recipes;
    expect(outputs.length).toBeGreaterThan(1);
    let selections: RecipeSelections = new Map();
    for (let i = 0; i < 20; i++) {
      selections = changeCraftingSelection(
        selections,
        initial.root.path,
        outputs[i % outputs.length].key,
      );
      const plan = buildCraftingPlan(graph, "Item", 17840106, (i % 2) + 1, { selections });
      expect(plan.nodeCount).toBeLessThan(1000);
      expect(selections.size).toBe(1);
      expect(
        Object.values(plan.shoppingList.Item).every(
          (quantity) => Number.isFinite(quantity) && quantity > 0,
        ),
      ).toBe(true);
    }
    expect(performance.now() - start).toBeLessThan(2000);
  },
  15000,
);
