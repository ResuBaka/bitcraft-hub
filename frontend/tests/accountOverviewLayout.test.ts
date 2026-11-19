import { describe, expect, test } from "bun:test";
import { parsePlayerLayout } from "../app/utils/accountOverviewLayout";

const valid = {
  format: "bitcraft-hub-player-layout",
  version: 1,
  blocks: [
    { id: "stats", x: 0, y: 0, w: 6, h: 3 },
    { id: "activity", x: 6, y: 0, w: 6, h: 3 },
  ],
};
const parse = (value: unknown) => parsePlayerLayout(JSON.stringify(value));

describe("shareable player layout", () => {
  test("accepts valid and empty layouts", () => {
    expect(parse(valid)).toEqual(valid);
    expect(parse({ ...valid, blocks: [] })?.blocks).toEqual([]);
  });
  test("rejects malformed, unsupported, overlapping and out-of-bounds layouts", () => {
    expect(parsePlayerLayout("{")).toBeNull();
    expect(parse({ ...valid, version: 2 })).toBeNull();
    expect(parse({ ...valid, blocks: [{ ...valid.blocks[0], id: "unknown" }] })).toBeNull();
    expect(parse({ ...valid, blocks: [valid.blocks[0], valid.blocks[0]] })).toBeNull();
    expect(parse({ ...valid, blocks: [valid.blocks[0], { ...valid.blocks[1], x: 5 }] })).toBeNull();
    expect(parse({ ...valid, blocks: [{ ...valid.blocks[0], w: 13 }] })).toBeNull();
    expect(parse({ ...valid, blocks: [{ ...valid.blocks[0], y: 1.5 }] })).toBeNull();
    expect(parsePlayerLayout("x".repeat(10001))).toBeNull();
  });
});
