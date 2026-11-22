import type { LeaderboardBoardDelta } from "../types/LeaderboardBoardDelta";
import type { LeaderboardBoardUpdate } from "../types/LeaderboardBoardUpdate";

type BoardRevision = {
  board_id: number;
  epoch: string;
  revision: number;
};

export function acceptsLeaderboardSnapshot(
  selectedBoard: number,
  current: BoardRevision | undefined,
  incoming: BoardRevision,
): boolean {
  if (incoming.board_id !== selectedBoard) return false;
  if (current?.board_id !== incoming.board_id || current.epoch !== incoming.epoch) return true;
  return incoming.revision > current.revision;
}

export function applyLeaderboardDelta(
  selectedBoard: number,
  current: LeaderboardBoardUpdate | undefined,
  incoming: LeaderboardBoardDelta,
): { status: "ignored" | "resync" } | { status: "applied"; board: LeaderboardBoardUpdate } {
  if (incoming.board_id !== selectedBoard) return { status: "ignored" };
  if (current?.board_id !== incoming.board_id || current.epoch !== incoming.epoch)
    return { status: "resync" };
  if (incoming.revision <= current.revision) return { status: "ignored" };
  if (incoming.base_revision !== current.revision) return { status: "resync" };

  const rowsById = new Map(current.rows.map((row) => [row.player_id, row]));
  const ids = current.rows.map((row) => row.player_id);
  for (const row of incoming.upserts) {
    if (!rowsById.has(row.player_id)) {
      if (!incoming.order) return { status: "resync" };
      ids.push(row.player_id);
    }
    rowsById.set(row.player_id, row);
  }
  const order = incoming.order ?? current.rows.map((_, index) => index);
  if (new Set(order).size !== order.length) return { status: "resync" };
  const rows: LeaderboardBoardUpdate["rows"] = [];
  for (const index of order) {
    const id = ids[index];
    const row = id === undefined ? undefined : rowsById.get(id);
    if (!row) return { status: "resync" };
    rows.push(row);
  }
  return { status: "applied", board: { ...current, revision: incoming.revision, rows } };
}
