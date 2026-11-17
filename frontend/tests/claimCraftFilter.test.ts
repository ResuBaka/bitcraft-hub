import { describe, expect, test } from "bun:test";
import { isCraftVisible } from "../app/utils/claimCraftFilter";

describe("claim craft visibility", () => {
  test("hides finished active crafts by default but keeps suspended crafts", () => {
    expect(isCraftVisible({ kind: "Active", status: "Complete", public: true }, false, false, false)).toBe(false);
    expect(isCraftVisible({ kind: "Active", status: "Suspended", public: true }, false, false, false)).toBe(true);
    expect(isCraftVisible({ kind: "Active", status: "Complete", public: true }, false, true, false)).toBe(true);
  });
  test("finished and public filters are independent, and passive completed projects are unaffected", () => {
    expect(isCraftVisible({ kind: "Active", status: "Complete", public: false }, false, true, false)).toBe(false);
    expect(isCraftVisible({ kind: "Active", status: "Complete", public: false }, true, true, false)).toBe(true);
    expect(isCraftVisible({ kind: "Passive", status: "Complete", public: null }, true, false, false)).toBe(false);
    expect(isCraftVisible({ kind: "Passive", status: "Complete", public: null }, false, false, true)).toBe(true);
  });
});
