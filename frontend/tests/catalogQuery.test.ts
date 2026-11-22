import { expect, test } from "bun:test";
import { catalogQueryPage, catalogQueryText } from "../app/utils/catalogQuery";

test("catalog query controls accept normalized scalar filters", () => {
  expect(catalogQueryText("  Copper ore  ")).toBe("Copper ore");
  expect(catalogQueryText("  ")).toBeUndefined();
  expect(catalogQueryText(["one", "two"])).toBeUndefined();
});

test("catalog pagination falls back to page one for invalid or multiple values", () => {
  expect(catalogQueryPage("12")).toBe(12);
  for (const value of [undefined, null, "0", "-3", "2.4", "NaN", ["2"], "99999999999999999"]) {
    expect(catalogQueryPage(value)).toBe(1);
  }
});
