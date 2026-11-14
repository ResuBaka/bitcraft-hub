import type { CargoDesc } from "../types/CargoDesc";
import type { ItemDesc } from "../types/ItemDesc";
import type { CraftingPlanNode, ItemType } from "./craftingPlan";

export type CraftingTreeItem = {
  path: string;
  label: string;
  node: CraftingPlanNode;
  description: ItemDesc | CargoDesc | undefined;
  children?: CraftingTreeItem[];
};
export type CraftingTreeView = {
  items: CraftingTreeItem[];
  byPath: ReadonlyMap<string, CraftingTreeItem>;
  parents: ReadonlyMap<string, string>;
  expandablePaths: string[];
};

// A lightweight UI projection of the CURRENT plan. Nodes and descriptions are
// referenced, never cloned; no graph alternatives or past plans are retained.
export function createCraftingTreeView(
  root: CraftingPlanNode,
  describe: (type: ItemType, id: number) => ItemDesc | CargoDesc | undefined,
): CraftingTreeView {
  const byPath = new Map<string, CraftingTreeItem>();
  const parents = new Map<string, string>();
  const expandablePaths: string[] = [];
  function visit(node: CraftingPlanNode, parent?: string): CraftingTreeItem {
    const description = describe(node.type, node.id);
    const item: CraftingTreeItem = {
      path: node.path,
      label: description?.name ?? `${node.type} #${node.id}`,
      node,
      description,
    };
    byPath.set(node.path, item);
    if (parent) parents.set(node.path, parent);
    if (node.children.length) {
      expandablePaths.push(node.path);
      item.children = node.children.map((child) => visit(child, node.path));
    }
    return item;
  }
  return { items: [visit(root)], byPath, parents, expandablePaths };
}

// A recipe switch may remove the selected ingredient. Fall back to its nearest
// surviving ancestor, not an unrelated occurrence with the same item ID.
export function resolveCraftingTreeSelection(view: CraftingTreeView, path: string): string {
  let candidate = path;
  while (candidate) {
    if (view.byPath.has(candidate)) return candidate;
    const separator = candidate.lastIndexOf("/");
    if (separator < 0) break;
    candidate = candidate.slice(0, separator);
  }
  return view.items[0]!.path;
}

export function craftingTreeAncestors(view: CraftingTreeView, path: string): string[] {
  const result: string[] = [];
  let parent = view.parents.get(path);
  while (parent) {
    result.push(parent);
    parent = view.parents.get(parent);
  }
  return result.reverse();
}

export function pruneCraftingTreeExpanded(
  view: CraftingTreeView,
  paths: readonly string[],
): string[] {
  return [...new Set(paths)].filter((path) => view.byPath.get(path)?.children?.length);
}

export function visibleCraftingTreePaths(
  view: CraftingTreeView,
  expandedPaths: readonly string[],
): string[] {
  const expanded = new Set(expandedPaths);
  const result: string[] = [];
  function visit(item: CraftingTreeItem) {
    result.push(item.path);
    if (expanded.has(item.path)) for (const child of item.children ?? []) visit(child);
  }
  for (const item of view.items) visit(item);
  return result;
}

const number = new Intl.NumberFormat(undefined, { maximumFractionDigits: 3 });
const percent = new Intl.NumberFormat(undefined, { style: "percent", maximumSignificantDigits: 4 });
const highPercent = new Intl.NumberFormat(undefined, {
  style: "percent",
  maximumFractionDigits: 2,
});
export const formatCraftingQuantity = (quantity: number, estimated = false) =>
  `${estimated ? "≈ " : ""}${number.format(quantity)}`;
export function formatCraftingChance(probability: number | null): string {
  if (probability === null) return "Unknown odds";
  const text = percent.format(probability);
  return probability < 1 && text === percent.format(1) ? `>${highPercent.format(0.9999)}` : text;
}
