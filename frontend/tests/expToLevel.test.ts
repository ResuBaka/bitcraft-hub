import { expect, test } from "bun:test";
import { levelMap } from "../app/consts/exp";
import { expToLevel } from "../app/utils";

test("XP crosses each level at its exact threshold", () => {
  for (const [level, xp] of Object.entries(levelMap)) {
    expect(expToLevel(xp - 0.5)).toBe(Number(level) - 1);
    expect(expToLevel(xp)).toBe(Number(level));
  }
  expect(expToLevel(Number.MAX_VALUE)).toBe(110);
  for (const invalid of [null, -1, NaN, Infinity, -Infinity]) {
    expect(expToLevel(invalid)).toBe(0);
  }
});
