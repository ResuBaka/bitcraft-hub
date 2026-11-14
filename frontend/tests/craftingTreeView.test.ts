import { expect, test } from "bun:test";
import {
  buildCraftingGraph,
  buildCraftingPlan,
  changeCraftingSelection,
} from "../app/utils/craftingPlan";
import {
  createCraftingTreeView,
  resolveCraftingTreeSelection,
  pruneCraftingTreeExpanded,
  craftingTreeAncestors,
  visibleCraftingTreePaths,
} from "../app/utils/craftingTreeView";
import type { RecipesAllResponse } from "../app/types/RecipesAllResponse";

function fixture(): RecipesAllResponse {
  const recipe = (id: number, output: number, inputs: number[]) => ({
    id,
    name: `Craft ${id}`,
    actions_required: 1,
    consumed_item_stacks: inputs.map((item_id) => ({ item_id, item_type: "Item", quantity: 1 })),
    crafted_item_stacks: [{ item_id: output, item_type: "Item", quantity: 1 }],
  });
  return {
    recipes: {
      1: recipe(1, 10, [20, 20]),
      2: recipe(2, 20, [30]),
      3: recipe(3, 20, [40]),
      4: recipe(4, 10, [50]),
    },
    item_desc: {
      10: { name: "Target", tier: 3, rarity: "Rare" },
      20: { name: "Duplicate ingredient", tier: 2, rarity: "Common" },
    },
    cargo_desc: {},
    item_list_desc: {},
  } as unknown as RecipesAllResponse;
}

test("tree rows reference only the current plan and source descriptions", () => {
  const data = fixture();
  const graph = buildCraftingGraph(data);
  const plan = buildCraftingPlan(graph, "Item", 10, 1, {
    selections: new Map([["Item:10", "1:1"]]),
  });
  const view = createCraftingTreeView(plan.root, (_type, id) => data.item_desc[id]);
  expect(view.byPath.size).toBe(plan.nodeCount);
  expect(view.items[0].node).toBe(plan.root);
  expect(view.items[0].description).toBe(data.item_desc[10]);
  expect(view.items[0].children![0].description).toBe(data.item_desc[20]);
  expect([...view.byPath.values()].some((row) => row.node.id === 40 || row.node.id === 50)).toBe(
    false,
  );
  expect(view.items[0].children![0].children![0].label).toBe("Item #30");
});

test("repeated item IDs have independent occurrence keys and parent chains", () => {
  const data = fixture();
  const plan = buildCraftingPlan(buildCraftingGraph(data), "Item", 10, 1, {
    selections: new Map([["Item:10", "1:1"]]),
  });
  const view = createCraftingTreeView(plan.root, (_type, id) => data.item_desc[id]);
  const [first, second] = view.items[0].children!;
  expect(first.node.id).toBe(second.node.id);
  expect(first.path).not.toBe(second.path);
  expect(view.byPath.get(first.path)).toBe(first);
  expect(view.byPath.get(second.path)).toBe(second);
  expect(craftingTreeAncestors(view, second.children![0].path)).toEqual([
    plan.root.path,
    second.path,
  ]);
});

test("recipe replacement prunes expanded branches and selects the nearest surviving ancestor", () => {
  const data = fixture();
  const graph = buildCraftingGraph(data);
  let selections = new Map([["Item:10", "1:1"]]);
  const old = buildCraftingPlan(graph, "Item", 10, 1, { selections });
  const oldView = createCraftingTreeView(old.root, (_type, id) => data.item_desc[id]);
  const first = old.root.children[0];
  const selected = first.children[0].path;
  selections = new Map(changeCraftingSelection(selections, first.path, "3:1"));
  const next = buildCraftingPlan(graph, "Item", 10, 1, { selections });
  const view = createCraftingTreeView(next.root, (_type, id) => data.item_desc[id]);
  expect(resolveCraftingTreeSelection(view, selected)).toBe(first.path);
  expect(
    pruneCraftingTreeExpanded(view, [...oldView.expandablePaths, selected, first.path]),
  ).toEqual(oldView.expandablePaths);
  const replaced = buildCraftingPlan(graph, "Item", 10, 1, {
    selections: new Map([["Item:10", "4:1"]]),
  });
  const replacedView = createCraftingTreeView(replaced.root, (_type, id) => data.item_desc[id]);
  expect(resolveCraftingTreeSelection(replacedView, selected)).toBe(replaced.root.path);
  expect(pruneCraftingTreeExpanded(replacedView, oldView.expandablePaths)).toEqual([
    replaced.root.path,
  ]);
});

test("stock updates preserve paths while replacing quantities and dropping covered descendants", () => {
  const data = fixture();
  const graph = buildCraftingGraph(data);
  const selections = new Map([["Item:10", "1:1"]]);
  const old = buildCraftingPlan(graph, "Item", 10, 1, { selections });
  const larger = buildCraftingPlan(graph, "Item", 10, 3, { selections });
  const largerView = createCraftingTreeView(larger.root, (_type, id) => data.item_desc[id]);
  expect(largerView.byPath.get(old.root.children[0].path)!.node.requested).toBe(3);
  expect(largerView.byPath.get(old.root.children[0].path)!.node).not.toBe(old.root.children[0]);
  const stocked = buildCraftingPlan(graph, "Item", 10, 1, {
    selections,
    inventory: { Item: { 10: 1 }, Cargo: {} },
  });
  const view = createCraftingTreeView(stocked.root, (_type, id) => data.item_desc[id]);
  expect(view.byPath.size).toBe(1);
  expect(resolveCraftingTreeSelection(view, old.root.children[0].path)).toBe(stocked.root.path);
  expect(pruneCraftingTreeExpanded(view, [old.root.path])).toEqual([]);
});

test("visible-path indexing respects expansion without traversing graph alternatives", () => {
  const data = fixture();
  const plan = buildCraftingPlan(buildCraftingGraph(data), "Item", 10, 1, {
    selections: new Map([["Item:10", "1:1"]]),
  });
  const view = createCraftingTreeView(plan.root, (_type, id) => data.item_desc[id]);
  expect(visibleCraftingTreePaths(view, [])).toEqual([plan.root.path]);
  expect(visibleCraftingTreePaths(view, [plan.root.path])).toEqual([
    plan.root.path,
    ...plan.root.children.map((child) => child.path),
  ]);
  expect(visibleCraftingTreePaths(view, view.expandablePaths)).toHaveLength(plan.nodeCount);
  expect(resolveCraftingTreeSelection(view, "Cargo:99/1:1/0")).toBe(plan.root.path);
});
