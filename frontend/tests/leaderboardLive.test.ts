import { describe, expect, test } from "bun:test";
import { acceptsLeaderboardSnapshot, applyLeaderboardDelta } from "../app/utils/leaderboardLive";
import type { LeaderboardBoardDelta } from "../app/types/LeaderboardBoardDelta";
import type { LeaderboardBoardUpdate } from "../app/types/LeaderboardBoardUpdate";

describe("live leaderboard snapshots", () => {
  const current = { board_id: 11, epoch: "first", revision: 3 };

  test("ignores a frame queued for a different board after switching tabs", () => {
    expect(acceptsLeaderboardSnapshot(42, undefined, current)).toBe(false);
  });

  test("does not overwrite a board with a duplicate or an older frame", () => {
    expect(acceptsLeaderboardSnapshot(11, current, current)).toBe(false);
    expect(acceptsLeaderboardSnapshot(11, current, { ...current, revision: 2 })).toBe(false);
    expect(acceptsLeaderboardSnapshot(11, current, { ...current, revision: 4 })).toBe(true);
  });

  test("accepts a new server epoch or the initial frame for another board", () => {
    expect(acceptsLeaderboardSnapshot(11, current, { ...current, epoch: "second", revision: 1 })).toBe(true);
    expect(acceptsLeaderboardSnapshot(42, current, { ...current, board_id: 42, revision: 1 })).toBe(true);
  });
});

describe("live leaderboard deltas", () => {
  const first = { player_id: "9007199254740993", player_name: "First", signed_in: true, experience: 30n };
  const second = { player_id: "9007199254740994", player_name: "Second", signed_in: false, experience: 25n };
  const third = { player_id: "9007199254740995", player_name: "Third", signed_in: true, experience: 20n };
  const current: LeaderboardBoardUpdate = {
    board_id: 11,
    epoch: "server",
    revision: 3,
    rows: [first, second],
  };
  const delta: LeaderboardBoardDelta = {
    board_id: 11,
    epoch: "server",
    base_revision: 3,
    revision: 4,
    upserts: [{ ...first, experience: 31n }],
  };

  test("applies only changed rows without mutating the current board", () => {
    const result = applyLeaderboardDelta(11, current, delta);
    expect(result.status).toBe("applied");
    if (result.status !== "applied") return;
    expect(result.board.revision).toBe(4);
    expect(result.board.rows).toEqual([delta.upserts[0], second]);
    expect(result.board.rows[1]).toBe(second);
    expect(current.rows[0]).toBe(first);
  });

  test("uses the provided order for reordering, new entries, and removals", () => {
    const result = applyLeaderboardDelta(11, current, {
      ...delta,
      revision: 7,
      upserts: [third],
      order: [2, 0],
    });
    expect(result).toEqual({
      status: "applied",
      board: { ...current, revision: 7, rows: [third, first] },
    });
  });

  test("ignores stale and unrelated frames; resyncs on a missing baseline", () => {
    expect(applyLeaderboardDelta(42, current, delta)).toEqual({ status: "ignored" });
    expect(applyLeaderboardDelta(11, current, { ...delta, revision: 3 })).toEqual({ status: "ignored" });
    expect(applyLeaderboardDelta(11, undefined, delta)).toEqual({ status: "resync" });
    expect(applyLeaderboardDelta(11, current, { ...delta, epoch: "restart" })).toEqual({ status: "resync" });
    expect(applyLeaderboardDelta(11, current, { ...delta, base_revision: 4, revision: 5 })).toEqual({ status: "resync" });
    expect(applyLeaderboardDelta(11, current, { ...delta, order: [42] })).toEqual({ status: "resync" });
  });
});
