import { describe, expect, test } from "bun:test";
import { computed, markRaw, reactive, ref, shallowRef, watch } from "vue";
import type { CraftingRecipe } from "../app/types/CraftingRecipe";
import type { RecipesAllResponse } from "../app/types/RecipesAllResponse";
import type { ExtractionRecipeResponse } from "../app/types/ExtractionRecipeResponse";
import type { ItemType, CraftingPlanNode } from "../app/utils/craftingPlan";
import * as crafting from "../app/utils/craftingPlan";
import { getItemOperationRecipes } from "../app/utils/itemOperations";

function recipe(
  id: number,
  output: number,
  inputs: [number, number, ItemType?][],
  quantity = 1,
  type: ItemType = "Item",
  name = `Recipe ${id}`,
): CraftingRecipe {
  return {
    id,
    name,
    actions_required: 2,
    crafted_item_stacks: [{ item_id: output, item_type: type, quantity, durability: null }],
    consumed_item_stacks: inputs.map(([item_id, quantity, item_type = "Item"]) => ({
      item_id,
      quantity,
      item_type,
      discovery_score: 0n,
      consumption_chance: 1,
    })),
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
function nodes(root: CraftingPlanNode): CraftingPlanNode[] {
  return [root, ...root.children.flatMap(nodes)];
}

describe("normalized crafting graph", () => {
  test("default route accounts for rare prerequisites, not just final-step odds", () => {
    const input = data(
      recipe(1, 100, [[20, 1]]),
      recipe(2, 200, [[30, 1]]),
      recipe(3, 300, [[40, 1]]),
      recipe(4, 30, [[40, 1]]),
    );
    for (const [wrapper, target, probability] of [
      [100, 10, 0.95],
      [200, 10, 0.5],
      [300, 20, 0.1],
    ]) {
      input.item_desc[wrapper] = { item_list_id: wrapper } as any;
      input.item_list_desc[wrapper] = {
        possibilities: [
          { probability, items: [{ item_id: target, item_type: "Item", quantity: 1 }] },
        ],
      } as any;
    }
    const graph = crafting.buildCraftingGraph(input);
    expect(graph.items.get("Item:10")!.defaultRecipe).toBe("2:1");
    const plan = crafting.buildCraftingPlan(graph, "Item", 10, 1);
    expect(plan.root.probability).toBe(0.5);
    expect(plan.root.totalActions).toBeCloseTo(8);
    const expensive = crafting.buildCraftingPlan(graph, "Item", 10, 1, {
      selections: new Map([["Item:10", "1:1"]]),
    });
    expect(expensive.root.totalActions).toBeGreaterThan(plan.root.totalActions);
  });
  test("stores one canonical recipe across multiple outputs and item lists", () => {
    const source = recipe(1, 10, [[30, 2]]);
    source.crafted_item_stacks.push({
      item_id: 20,
      item_type: "Cargo",
      quantity: 3,
      durability: null,
    });
    const input = data(source);
    input.item_desc[10] = { item_list_id: 1 } as any;
    input.item_list_desc[1] = {
      possibilities: [
        {
          probability: 1,
          items: [
            { item_id: 10, item_type: "Item", quantity: 1 },
            { item_id: 20, item_type: "Cargo", quantity: 3 },
          ],
        },
      ],
    } as any;
    const graph = crafting.buildCraftingGraph(input);
    expect(graph.recipes.size).toBe(1);
    expect(graph.items.get("Item:10")!.recipes).toHaveLength(1);
    expect(graph.items.get("Cargo:20")!.recipes).toHaveLength(1);
    expect(graph.items.get("Cargo:20")!.recipes[0].recipeId).toBe(
      graph.items.get("Item:10")!.recipes[0].recipeId,
    );
    expect(graph.recipes.get(1)!.ingredients).toHaveLength(1);
  });

  test("keeps Item and Cargo IDs separate", () => {
    const graph = crafting.buildCraftingGraph(
      data(recipe(1, 10, [[20, 1]]), recipe(2, 10, [[30, 1]], 1, "Cargo")),
    );
    expect(crafting.buildCraftingPlan(graph, "Item", 10, 1).shoppingList.Item).toEqual({ 20: 1 });
    expect(crafting.buildCraftingPlan(graph, "Cargo", 10, 1).shoppingList.Item).toEqual({ 30: 1 });
  });

  test("excludes Package recipes and ingredients", () => {
    const input = data(
      recipe(1, 10, [[20, 1]], 1, "Item", "Package thing"),
      recipe(2, 10, [
        [30, 1],
        [40, 1],
      ]),
    );
    input.item_desc[30] = { name: "Package", item_list_id: 0 } as any;
    const graph = crafting.buildCraftingGraph(input);
    expect(graph.recipes.size).toBe(1);
    expect(crafting.buildCraftingPlan(graph, "Item", 10, 1).shoppingList.Item).toEqual({ 40: 1 });
  });

  test("defaults to a productive route instead of self-recrafting", () => {
    const graph = crafting.buildCraftingGraph(
      data(
        recipe(1, 10, [
          [10, 1],
          [30, 1],
        ]),
        recipe(2, 10, [[20, 1]]),
      ),
    );
    expect(graph.items.get("Item:10")!.recipes).toHaveLength(2);
    expect(graph.items.get("Item:10")!.defaultRecipe).toBe("2:1");
    expect(crafting.buildCraftingPlan(graph, "Item", 10, 1).shoppingList.Item).toEqual({ 20: 1 });
  });

  test("does not turn package-only unpacking into free production", () => {
    const input = data(recipe(1, 10, [[20, 1]], 1, "Item", "Unpack {1}"), recipe(2, 10, [[30, 2]]));
    input.item_desc[20] = { name: "Ingot Package", item_list_id: 0 } as any;
    const graph = crafting.buildCraftingGraph(input);
    expect(graph.recipes.has(1)).toBe(false);
    expect(graph.items.get("Item:10")!.defaultRecipe).toBe("2:1");
    expect(crafting.buildCraftingPlan(graph, "Item", 10, 1).shoppingList.Item).toEqual({ 30: 2 });
  });

  test("gather-only materials have a supply default, with crafting still selectable", () => {
    const input = data(recipe(1, 10, [[20, 1]]));
    input.item_desc[10] = { name: "Fine Animal Hair", item_list_id: 0 } as any;
    const graph = crafting.buildCraftingGraph(input);
    expect(crafting.buildCraftingPlan(graph, "Item", 10, 1).shoppingList.Item).toEqual({ 10: 1 });
    expect(
      crafting.buildCraftingPlan(graph, "Item", 10, 1, {
        selections: new Map([["Item:10", "1:1"]]),
      }).shoppingList.Item,
    ).toEqual({ 20: 1 });
  });
});

describe("selected crafting plan", () => {
  test("normal crafting compounds rarity retries through previous-tier ingredients", () => {
    const input = data(recipe(1, 100, [[20, 1]]), recipe(2, 200, [[30, 1]]));
    input.item_desc[100] = { item_list_id: 100 } as any;
    input.item_desc[200] = { item_list_id: 200 } as any;
    input.item_list_desc[100] = {
      possibilities: [
        { probability: 0.5, items: [{ item_id: 10, item_type: "Item", quantity: 1 }] },
        { probability: 0.5, items: [{ item_id: 11, item_type: "Item", quantity: 1 }] },
      ],
    } as any;
    input.item_list_desc[200] = {
      possibilities: [
        { probability: 0.25, items: [{ item_id: 20, item_type: "Item", quantity: 1 }] },
        { probability: 0.75, items: [{ item_id: 21, item_type: "Item", quantity: 1 }] },
      ],
    } as any;
    const graph = crafting.buildCraftingGraph(input);
    const plan = crafting.buildCraftingPlan(graph, "Item", 10, 1);
    expect(plan.root.probability).toBe(0.5);
    expect(plan.root.attempts).toBe(2);
    expect(plan.root.children[0].requested).toBe(2);
    expect(plan.root.children[0].probability).toBe(0.25);
    expect(plan.root.children[0].attempts).toBe(8);
    expect(plan.shoppingList.Item).toEqual({ 30: 8 });
    expect(plan.root.totalActions).toBe(20);
    expect(plan.probabilityAdjusted).toBe(true);
    // Other rarity IDs never substitute for the required previous rarity.
    const wrongRarity = crafting.buildCraftingPlan(graph, "Item", 10, 1, {
      inventory: { Item: { 21: 100 }, Cargo: {} },
    });
    expect(wrongRarity.shoppingList.Item).toEqual({ 30: 8 });
    const correctRarity = crafting.buildCraftingPlan(graph, "Item", 10, 1, {
      inventory: { Item: { 20: 1 }, Cargo: {} },
    });
    expect(correctRarity.shoppingList.Item).toEqual({ 30: 4 });
  });

  test("fractional average retries are not rounded at every rarity stage", () => {
    const input = data(
      recipe(1, 100, [[20, 1]]),
      recipe(2, 200, [[30, 1]]),
      recipe(3, 300, [[40, 1]]),
    );
    for (const [wrapper, target] of [
      [100, 10],
      [200, 20],
      [300, 30],
    ]) {
      input.item_desc[wrapper] = { item_list_id: wrapper } as any;
      input.item_list_desc[wrapper] = {
        possibilities: [
          { probability: 0.9, items: [{ item_id: target, item_type: "Item", quantity: 1 }] },
        ],
      } as any;
    }
    const plan = crafting.buildCraftingPlan(crafting.buildCraftingGraph(input), "Item", 10, 1);
    expect(plan.root.attempts).toBeCloseTo(1 / 0.9);
    expect(plan.root.children[0].attempts).toBeCloseTo(1 / 0.9 ** 2);
    expect(plan.root.children[0].children[0].attempts).toBeCloseTo(1 / 0.9 ** 3);
    expect(plan.root.children[0].children[0].children[0].requested).toBeCloseTo(1 / 0.9 ** 3);
    expect(plan.shoppingList.Item).toEqual({ 40: 2 });
    expect(plan.nodeCount).toBe(4);
  });

  test("a chance-based batch of four still needs a successful roll for one item", () => {
    const input = data(recipe(1, 100, [[30, 2]]));
    input.item_desc[100] = { item_list_id: 100 } as any;
    input.item_list_desc[100] = {
      possibilities: [
        { probability: 0.25, items: [{ item_id: 10, item_type: "Item", quantity: 4 }] },
      ],
    } as any;
    const graph = crafting.buildCraftingGraph(input);
    const plan = crafting.buildCraftingPlan(graph, "Item", 10, 1);
    expect(plan.root.expectedYield).toBe(1);
    expect(plan.root.attempts).toBeCloseTo(4);
    expect(plan.shoppingList.Item).toEqual({ 30: 8 });
    expect(crafting.buildCraftingPlan(graph, "Item", 10, 5).root.attempts).toBeCloseTo(8);
  });

  test("variable-sized drops use exact one-drop odds and label larger yield approximations", () => {
    const input = data(recipe(1, 100, [[30, 1]]));
    input.item_desc[100] = { item_list_id: 100 } as any;
    input.item_list_desc[100] = {
      possibilities: [
        { probability: 0.1, items: [{ item_id: 10, item_type: "Item", quantity: 2 }] },
        { probability: 0.1, items: [{ item_id: 10, item_type: "Item", quantity: 4 }] },
      ],
    } as any;
    const graph = crafting.buildCraftingGraph(input);
    const one = crafting.buildCraftingPlan(graph, "Item", 10, 1);
    expect(one.root.attempts).toBeCloseTo(5);
    expect(one.root.variableYield).toBe(true);
    expect(one.root.averageEstimate).toBe(true);
    expect(graph.items.get("Item:10")!.craftingRecipes).toHaveLength(1);
  });

  test("zero-chance rarities are not registered as guaranteed craft outputs", () => {
    const input = data(recipe(1, 100, [[30, 1]]));
    input.item_desc[100] = { item_list_id: 100 } as any;
    input.item_list_desc[100] = {
      possibilities: [
        { probability: 0, items: [{ item_id: 10, item_type: "Item", quantity: 1 }] },
        { probability: 1, items: [{ item_id: 20, item_type: "Item", quantity: 1 }] },
      ],
    } as any;
    const graph = crafting.buildCraftingGraph(input);
    expect(graph.items.get("Item:10")).toBeUndefined();
    expect(graph.items.get("Item:20")!.craftingRecipes[0].probability).toBe(1);
  });

  test("scales ingredient quantities and computes totals once bottom-up", () => {
    const graph = crafting.buildCraftingGraph(data(recipe(1, 10, [[20, 3]], 2)));
    const plan = crafting.buildCraftingPlan(graph, "Item", 10, 5, {
      extraction: { "Item:20": 0.4 },
    });
    expect(plan.root.actions).toBe(6);
    expect(plan.root.children[0].quantity).toBe(8);
    expect(plan.root.totalActions).toBe(10);
    expect(plan.shoppingList.Item).toEqual({ 20: 8 });
    expect(plan.nodeCount).toBe(2);
  });

  test("never calculates descendants of an unselected alternative", () => {
    const graph = crafting.buildCraftingGraph(
      data(recipe(1, 10, [[20, 1]]), recipe(2, 10, [[30, 1]]), recipe(3, 30, [[40, 1]])),
    );
    let reads = 0;
    Object.defineProperty(graph.recipes.get(3)!.ingredients[0], "quantity", {
      get() {
        reads++;
        return 1;
      },
    });
    const initial = crafting.buildCraftingPlan(graph, "Item", 10, 1);
    expect(initial.shoppingList.Item).toEqual({ 20: 1 });
    expect(reads).toBe(0);
    expect(initial.root.children).toHaveLength(1);
    const next = crafting.buildCraftingPlan(graph, "Item", 10, 1, {
      selections: new Map([["Item:10", "2:1"]]),
    });
    expect(next.shoppingList.Item).toEqual({ 40: 1 });
    expect(reads).toBe(1);
    expect(initial.shoppingList.Item).toEqual({ 20: 1 });
  });

  test("inventory deductions use shortage quantities at every level", () => {
    const graph = crafting.buildCraftingGraph(data(recipe(1, 10, [[20, 3]], 2)));
    const inventory = { Item: { 10: 1, 20: 2 }, Cargo: {} };
    const plan = crafting.buildCraftingPlan(graph, "Item", 10, 5, {
      inventory,
      extraction: { "Item:20": 0.5 },
    });
    expect(plan.root.owned).toBe(1);
    expect(plan.root.quantity).toBe(4);
    expect(plan.root.children[0].requested).toBe(6);
    expect(plan.root.children[0].owned).toBe(2);
    expect(plan.shoppingList.Item).toEqual({ 20: 4 });
    expect(plan.root.totalActions).toBe(6);
    expect(inventory).toEqual({ Item: { 10: 1, 20: 2 }, Cargo: {} });
  });

  test("does not reuse inventory across separate occurrences of the same material", () => {
    const graph = crafting.buildCraftingGraph(
      data(
        recipe(1, 10, [
          [20, 1],
          [30, 1],
        ]),
        recipe(2, 20, [[40, 2]]),
        recipe(3, 30, [[40, 3]]),
      ),
    );
    const plan = crafting.buildCraftingPlan(graph, "Item", 10, 1, {
      inventory: { Item: { 40: 4 }, Cargo: {} },
    });
    expect(plan.shoppingList.Item).toEqual({ 40: 1 });
    expect(plan.root.children[0].children[0].owned).toBe(2);
    expect(plan.root.children[1].children[0].owned).toBe(2);
  });

  test("fully stocked nodes do not allocate any descendant plan", () => {
    const graph = crafting.buildCraftingGraph(
      data(recipe(1, 10, [[20, 1]]), recipe(2, 20, [[30, 1]])),
    );
    const plan = crafting.buildCraftingPlan(graph, "Item", 10, 5, {
      inventory: { Item: { 10: 5 }, Cargo: {} },
    });
    expect(plan.nodeCount).toBe(1);
    expect(plan.root.children).toHaveLength(0);
    expect(plan.root.status).toBe("inventory");
    expect(plan.root.totalActions).toBe(0);
    expect(plan.shoppingList).toEqual({ Item: {}, Cargo: {} });
  });

  test("allows different recipes for two occurrences of the same item", () => {
    const graph = crafting.buildCraftingGraph(
      data(
        recipe(1, 10, [
          [20, 1],
          [20, 1],
        ]),
        recipe(2, 20, [[30, 1]]),
        recipe(3, 20, [[40, 2]]),
      ),
    );
    const initial = crafting.buildCraftingPlan(graph, "Item", 10, 1);
    const selections = new Map([[initial.root.children[0].path, "3:1"]]);
    const plan = crafting.buildCraftingPlan(graph, "Item", 10, 1, { selections });
    expect(plan.shoppingList.Item).toEqual({ 30: 1, 40: 2 });
    expect(plan.root.children[0].recipeId).toBe(3);
    expect(plan.root.children[1].recipeId).toBe(2);
  });

  test("explicit recrafting consumes starting equipment instead of treating it as finished", () => {
    const graph = crafting.buildCraftingGraph(
      data(
        recipe(
          1,
          10,
          [
            [10, 1],
            [20, 2],
          ],
          1,
          "Item",
          "Recraft {1}",
        ),
        recipe(2, 10, [[30, 1]]),
      ),
    );
    const selections = new Map([["Item:10", "1:1"]]);
    const supplied = crafting.buildCraftingPlan(graph, "Item", 10, 1, {
      selections,
      inventory: { Item: { 10: 1 }, Cargo: {} },
    });
    expect(supplied.root.status).toBe("craft");
    expect(supplied.root.children[0].status).toBe("inventory");
    expect(supplied.shoppingList.Item).toEqual({ 20: 2 });
    const missing = crafting.buildCraftingPlan(graph, "Item", 10, 1, { selections });
    expect(missing.root.children[0].status).toBe("requires-existing");
    expect(missing.shoppingList.Item).toEqual({ 10: 1, 20: 2 });
    expect(missing.nodeCount).toBe(3);
  });

  test("terminates indirect cycles and reports the existing input requirement", () => {
    const graph = crafting.buildCraftingGraph(
      data(recipe(1, 10, [[20, 1]]), recipe(2, 20, [[10, 1]])),
    );
    expect(crafting.buildCraftingPlan(graph, "Item", 10, 1).root.status).toBe("requires-existing");
    const selections = new Map([
      ["Item:10", "1:1"],
      ["Item:10/1:1/0", "2:1"],
    ]);
    const plan = crafting.buildCraftingPlan(graph, "Item", 10, 1, { selections });
    expect(plan.nodeCount).toBe(3);
    expect(plan.root.children[0].children[0].status).toBe("requires-existing");
    expect(plan.shoppingList.Item).toEqual({ 10: 1 });
  });

  test("can stop an otherwise craftable branch at a supplied base material", () => {
    const graph = crafting.buildCraftingGraph(data(recipe(1, 10, [[20, 1]])));
    const plan = crafting.buildCraftingPlan(graph, "Item", 10, 2, {
      selections: new Map([["Item:10", null]]),
    });
    expect(plan.nodeCount).toBe(1);
    expect(plan.shoppingList.Item).toEqual({ 10: 2 });
    expect(plan.root.totalActions).toBe(0);
  });

  test("supports recipes with no consumed ingredients", () => {
    const graph = crafting.buildCraftingGraph(data(recipe(1, 10, [])));
    const plan = crafting.buildCraftingPlan(graph, "Item", 10, 2);
    expect(plan.root.actions).toBe(4);
    expect(plan.shoppingList.Item).toEqual({});
  });

  test("plan records are serializable values, not closures or alternative caches", () => {
    const graph = crafting.buildCraftingGraph(
      data(recipe(1, 10, [[20, 1]]), recipe(2, 10, [[30, 1]])),
    );
    const plan = crafting.buildCraftingPlan(graph, "Item", 10, 1);
    expect(JSON.parse(JSON.stringify(plan))).toEqual(plan);
    expect(nodes(plan.root)).toHaveLength(plan.nodeCount);
    for (const node of nodes(plan.root)) {
      expect(
        Object.values(Object.getOwnPropertyDescriptors(node)).some(
          (descriptor) => descriptor.get || descriptor.set,
        ),
      ).toBe(false);
      expect(node.totalActions).toBe(
        node.actions + node.children.reduce((sum, child) => sum + child.totalActions, 0),
      );
    }
  });
});

describe("choice state and extraction", () => {
  test("replacing a branch removes descendant choices but preserves sibling choices", () => {
    const old = new Map<string, string | null>([
      ["Item:10", "1:1"],
      ["Item:10/1:1/0", "2:1"],
      ["Item:100", "4:1"],
    ]);
    const next = crafting.changeCraftingSelection(old, "Item:10", "3:1");
    expect([...next]).toEqual([
      ["Item:10", "3:1"],
      ["Item:100", "4:1"],
    ]);
    expect(old.size).toBe(3);
    expect(crafting.changeCraftingSelection(next, "Item:10", "3:1")).toBe(next);
  });

  test("repeated switching retains only choice IDs and the latest selected plan", () => {
    const graph = crafting.buildCraftingGraph(
      data(recipe(1, 10, [[20, 1]]), recipe(2, 10, [[30, 1]])),
    );
    let selections: crafting.RecipeSelections = new Map();
    for (let i = 0; i < 200; i++) {
      selections = crafting.changeCraftingSelection(selections, "Item:10", i % 2 ? "1:1" : "2:1");
      const plan = crafting.buildCraftingPlan(graph, "Item", 10, 1, { selections });
      expect(plan.nodeCount).toBe(2);
      expect(selections.size).toBe(1);
      expect(graph.recipes.size).toBe(2);
    }
  });

  test("sums expected list yields and selects the cheapest extraction source", () => {
    const input = data();
    input.item_desc[10] = { item_list_id: 1 } as any;
    input.item_list_desc[1] = {
      possibilities: [
        {
          probability: 0.5,
          items: [
            { item_id: 20, item_type: "Item", quantity: 2 },
            { item_id: 30, item_type: "Cargo", quantity: 1 },
          ],
        },
      ],
    } as any;
    const sources = [
      {
        extracted_item_stacks: [
          { probability: 0.25, item_stack: { item_id: 10, item_type: "Item", quantity: 4 } },
        ],
      },
      {
        extracted_item_stacks: [
          { probability: 1, item_stack: { item_id: 20, item_type: "Item", quantity: 10 } },
        ],
      },
    ] as ExtractionRecipeResponse[];
    expect(crafting.buildExtractionEffort(sources, input)).toEqual({
      "Item:20": 0.1,
      "Cargo:30": 2,
    });
  });
});

async function pageState(input: RecipesAllResponse) {
  const source = await Bun.file(
    new URL("../app/pages/items/[type]/[id].client.vue", import.meta.url),
  ).text();
  const script = source
    .match(/<script setup lang="ts">([\s\S]*?)<\/script>/)![1]
    .replace(/^import[\s\S]*?from ["'][^"']+["'];\s*/gm, "");
  const code = new Bun.Transpiler({ loader: "ts" }).transformSync(script);
  const route = reactive({ params: { id: "10", type: "Item" } });
  const calls = { graph: 0, plan: 0 };
  const dependencies = {
    getItemOperationRecipes,
    ...crafting,
    ref,
    shallowRef,
    computed,
    watch,
    markRaw,
    watchThrottled: () => {},
    useRoute: () => route,
    useSeoMeta: () => {},
    useLazyFetchMsPack: () => ({ data: ref(), refresh: () => {} }),
    useFetchMsPack: (url: () => string) => ({
      data: ref(url() === "/recipes/get_all" ? input : []),
    }),
    buildCraftingGraph: (data: RecipesAllResponse) => {
      calls.graph++;
      return crafting.buildCraftingGraph(data);
    },
    buildCraftingPlan: (...args: Parameters<typeof crafting.buildCraftingPlan>) => {
      calls.plan++;
      return crafting.buildCraftingPlan(...args);
    },
  };
  const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
  const page = await new AsyncFunction(
    ...Object.keys(dependencies),
    `${code}; return { plan, graph, amount, selections, selectRecipe, playerInventoryData, extractionRecipesFetch, shoppingItems };`,
  )(...Object.values(dependencies));
  return { ...page, calls, route };
}

describe("page integration", () => {
  test("shopping materials sort highest tier first, then name, across Item and Cargo", async () => {
    const input = data(
      recipe(1, 10, [
        [20, 1],
        [30, 1],
        [40, 1, "Cargo"],
        [50, 1, "Cargo"],
        [60, 1],
        [70, 1],
      ]),
    );
    input.item_desc[20] = { name: "Alpha metal", tier: 4, rarity: "Rare" } as any;
    input.item_desc[30] = { name: "Zulu fiber", tier: 2, rarity: "Common" } as any;
    input.item_desc[70] = { name: "Starter material", tier: 0, rarity: "Default" } as any;
    input.cargo_desc[40] = { name: "Oak trunk", tier: 1, rarity: "Common" } as any;
    input.cargo_desc[50] = { name: "Alpha trunk", tier: 2, rarity: "Common" } as any;
    const page = await pageState(input);
    expect(page.shoppingItems.value.map((item: { id: number }) => item.id)).toEqual([
      20, 50, 30, 40, 70, 60,
    ]);
    expect(page.shoppingItems.value.map((item: { quantity: number }) => item.quantity)).toEqual([
      1, 1, 1, 1, 1, 1,
    ]);
  });
  test("choices, amounts, and inventory changes reuse the same normalized graph", async () => {
    const page = await pageState(data(recipe(1, 10, [[20, 1]]), recipe(2, 10, [[30, 1]])));
    expect(page.plan.value.shoppingList.Item).toEqual({ 20: 1 });
    const graph = page.graph.value;
    page.selectRecipe("Item:10", "2:1");
    expect(page.plan.value.shoppingList.Item).toEqual({ 30: 1 });
    page.amount.value = 3;
    expect(page.plan.value.shoppingList.Item).toEqual({ 30: 3 });
    page.playerInventoryData.value = {
      inventorys: [{ pockets: [{ contents: { item_id: 30, item_type: "Item", quantity: 1 } }] }],
    };
    expect(page.plan.value.shoppingList.Item).toEqual({ 30: 2 });
    expect(page.graph.value).toBe(graph);
    expect(page.calls.graph).toBe(1);
  });

  test("navigation clears choices from the previous item", async () => {
    const page = await pageState(
      data(recipe(1, 10, [[20, 1]]), recipe(2, 10, [[30, 1]]), recipe(3, 40, [[50, 1]])),
    );
    page.selectRecipe("Item:10", "2:1");
    expect(page.plan.value.shoppingList.Item).toEqual({ 30: 1 });
    page.route.params.id = "40";
    expect(page.plan.value.shoppingList.Item).toEqual({ 50: 1 });
    expect(page.selections.value.size).toBe(0);
  });

  test("late extraction results update actions without losing the selected recipe", async () => {
    const page = await pageState(data(recipe(1, 10, [[20, 1]]), recipe(2, 10, [[30, 1]])));
    page.selectRecipe("Item:10", "2:1");
    expect(page.plan.value.root.totalActions).toBe(2);
    page.extractionRecipesFetch.value = [
      {
        extracted_item_stacks: [
          { probability: 0.25, item_stack: { item_id: 30, item_type: "Item", quantity: 1 } },
        ],
      },
    ];
    expect(page.plan.value.root.totalActions).toBe(6);
    expect(page.plan.value.root.recipeId).toBe(2);
    expect(page.calls.graph).toBe(1);
  });
});
