import { expect, test } from "bun:test";
import { describeObservation } from "../app/utils/watchtowerHistory";
import type { WatchtowerReading } from "../app/types/WatchtowerReading";
import type { WatchtowerObservation } from "../app/types/WatchtowerObservation";

const reading: WatchtowerReading = {
  empire_id: "12", empire_name: "North", active: true, energy: 100, upkeep: 5,
  x: 18, z: 90, dimension: 1n, start_timestamp: null,
};
const observation: WatchtowerObservation = {
  id: "34", observed_at: "2026-11-24T12:00:00Z", source: "tower", event: "changed",
  previous: reading, current: { ...reading, empire_id: "24", empire_name: null, energy: 50 },
};
test("ownership and energy changes are both visible when an empire name is unavailable", () => {
  expect(describeObservation(observation)).toEqual({
    title: "Ownership changed", details: ["North → Empire 24", "Energy 100 → 50"],
  });
});
test("a baseline is not a build and a removed mark is not a lost siege", () => {
  expect(describeObservation({ ...observation, previous: null, event: "baseline" }).title).toBe("Watchtower baseline");
  expect(describeObservation({
    ...observation, source: "siege", event: "removed", previous: { ...reading, active: false }, current: null,
  }).title).toBe("Siege mark removed");
});
