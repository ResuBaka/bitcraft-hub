import { describe, expect, test } from "bun:test";
import {
  accountOverviewBlocks,
  blockDropPosition,
  isAccountOverviewBlockId,
} from "../app/utils/accountOverviewBlocks";

describe("account overview blocks", () => {
  test("accepts only supported block IDs", () => {
    for (const block of accountOverviewBlocks)
      expect(isAccountOverviewBlockId(block.id)).toBe(true);
    expect(isAccountOverviewBlockId("unknown")).toBe(false);
  });

  test("places a dropped block within the grid", () => {
    const rect = { left: 100, top: 200, width: 600 };
    expect(blockDropPosition(250, 280, rect)).toEqual({ x: 3, y: 2 });
    expect(blockDropPosition(900, 100, rect)).toEqual({ x: 6, y: 0 });
    expect(blockDropPosition(0, 200, { ...rect, width: 0 })).toEqual({ x: 0, y: 0 });
  });
});
