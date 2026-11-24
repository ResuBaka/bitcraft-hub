import { expect, test } from "bun:test";
import { empireLocation, formatEmpireNewsArgument } from "../app/utils/empireLocation";

test("tower location and game report identify the same map tile", () => {
  expect(empireLocation(18205, 11721)).toBe("N: 3907 E: 6068");
  expect(formatEmpireNewsArgument("N:{0}, E:{1}|~3907|~6068")).toBe(empireLocation(18205, 11721));
});

test("row offsets are removed before rounding to the parent tile", () => {
  expect(empireLocation(2, 3)).toBe("N: 1 E: 0");
  expect(empireLocation(-2, -3)).toBe("N: -1 E: 0");
});
