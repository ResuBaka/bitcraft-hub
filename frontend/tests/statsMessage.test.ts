import { describe, expect, test } from "bun:test";
import { statsMessageAction } from "../app/utils/statsMessage";
import type { StatsSnapshot } from "../app/types/StatsSnapshot";

const snapshot: StatsSnapshot = {
  player_id: "42",
  available: true,
  health: 50,
  stamina: 25,
  buffs: [],
  buffs_complete: true,
};

describe("stats websocket messages", () => {
  test("uses a matching snapshot directly without requesting a refresh", () => {
    expect(statsMessageAction("42", { player_id: "42", stats: snapshot })).toEqual({
      type: "snapshot",
      snapshot,
    });
  });

  test("ignores other players and refreshes for invalidations or older servers", () => {
    expect(statsMessageAction("42", { player_id: "43" })).toEqual({ type: "ignore" });
    expect(statsMessageAction("42", { player_id: null })).toEqual({ type: "refresh" });
    expect(statsMessageAction("42", { player_id: "42" })).toEqual({ type: "refresh" });
    expect(
      statsMessageAction("42", { player_id: "42", stats: { ...snapshot, player_id: "43" } }),
    ).toEqual({ type: "refresh" });
  });
});
