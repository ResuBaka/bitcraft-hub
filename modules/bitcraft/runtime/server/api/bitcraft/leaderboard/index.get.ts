import {
  XPToLevel,
  type ExtendedExpeirenceStateRow,
  getLeaderboard,
} from "~/modules/bitcraft/gamestate/experienceState";

export interface ExtendedExpeirenceStateRowWithName
  extends ExtendedExpeirenceStateRow {
  entity_name?: string;
}

export default defineEventHandler((event) => {
  const response = {};

  const leaderboardState = getLeaderboard();

  for (const key in leaderboardState) {
    response[key] = leaderboardState[key].slice(0, 100);
  }

  return {
    ...response,
  };
});
