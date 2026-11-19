import type { StatsSnapshot } from "~/types/StatsSnapshot";

export type StatsMessage = { player_id: string | null; stats?: StatsSnapshot | null };
export type StatsMessageAction =
  | { type: "ignore" }
  | { type: "refresh" }
  | { type: "snapshot"; snapshot: StatsSnapshot };

export function statsMessageAction(playerId: string, message: StatsMessage): StatsMessageAction {
  if (message.player_id !== null && message.player_id !== playerId) return { type: "ignore" };
  if (message.player_id === playerId && message.stats?.player_id === playerId) {
    return { type: "snapshot", snapshot: message.stats };
  }
  return { type: "refresh" };
}
