import { describe, expect, test } from "bun:test";
import { groupCraftRows } from "../app/utils/passiveCraftGroups";

describe("passive craft grouping", () => {
  test("groups by recipe, owner and status without merging active crafts", () => {
    const passive = { id: "1", kind: "Passive" as const, recipe_id: 10, owner_id: "20", status: "Queued" };
    const crafts = [
      passive,
      { ...passive, id: "2" },
      { ...passive, id: "3", owner_id: "21" },
      { ...passive, id: "4", status: "Processing" },
      { ...passive, id: "5", recipe_id: 11 },
      { ...passive, id: "6", kind: "Active" as const },
      { ...passive, id: "7", kind: "Active" as const },
    ];
    expect(groupCraftRows(crafts).map((group) => group.map((craft) => craft.id))).toEqual([
      ["1", "2"], ["3"], ["4"], ["5"], ["6"], ["7"],
    ]);
  });
});
