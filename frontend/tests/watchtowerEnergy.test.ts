import { expect, test } from "bun:test";
import { watchtowerEmptyBy } from "../app/utils/watchtowerEnergy";

const tower = { active: true, energy: 60, upkeep: 30, under_siege: false, decay_tick_millis: 3_600_000 };
const reading = Date.UTC(2026, 11, 31, 22, 30);

test("depletion occurs on the final upkeep tick even with a partial charge", () => {
  expect(watchtowerEmptyBy(tower, reading)).toEqual(new Date("2027-01-01T00:30:00Z"));
  expect(watchtowerEmptyBy({ ...tower, energy: 61 }, reading)).toEqual(new Date("2027-01-01T01:30:00Z"));
  expect(watchtowerEmptyBy({ ...tower, energy: 1, decay_tick_millis: 90_000 }, reading)).toEqual(new Date("2026-12-31T22:32:00Z"));
});

test("distinguishes depleted towers, paused upkeep, and missing game timing", () => {
  expect(watchtowerEmptyBy({ ...tower, energy: 0, active: false }, reading)).toBe("Depleted");
  expect(watchtowerEmptyBy({ ...tower, active: false }, reading)).toBe("Not draining (inactive)");
  expect(watchtowerEmptyBy({ ...tower, under_siege: true }, reading)).toBe("Paused during siege");
  expect(watchtowerEmptyBy({ ...tower, upkeep: 0 }, reading)).toBe("No upkeep");
  expect(watchtowerEmptyBy({ ...tower, decay_tick_millis: null }, reading)).toBe("Estimate unavailable");
  expect(watchtowerEmptyBy(tower, null)).toBe("Estimate unavailable");
});
