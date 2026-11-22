import { expect, test } from "bun:test";
import { applyCharacterStatsDelta } from "../app/utils/characterStatsLive";

const snapshot = { player_id: "42", revision: 12, values: [100, 20, 5] };
const delta = {
  base_revision: 12,
  revision: 19,
  changes: [
    [1, 0],
    [2, null],
  ] as [number, number | null][],
};

test("applies only changed slots without mutating the previous snapshot", () => {
  expect(applyCharacterStatsDelta(snapshot, delta)).toEqual({
    status: "applied",
    snapshot: { player_id: "42", revision: 19, values: [100, 0, null] },
  });
  expect(snapshot.values).toEqual([100, 20, 5]);
});

test("ignores older patches and recovers from gaps, invalid indices, or missing baselines", () => {
  expect(applyCharacterStatsDelta({ ...snapshot, revision: 19 }, delta)).toEqual({
    status: "ignored",
  });
  expect(applyCharacterStatsDelta({ ...snapshot, revision: 11 }, delta)).toEqual({
    status: "resync",
  });
  expect(applyCharacterStatsDelta(null, delta)).toEqual({ status: "resync" });
  expect(applyCharacterStatsDelta(snapshot, { ...delta, changes: [[3, 1]] })).toEqual({
    status: "resync",
  });
});
