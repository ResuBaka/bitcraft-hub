import {
  type ExtendedExpeirenceStateRow,
  getLeaderboard,
} from "~/modules/bitcraft/gamestate/experienceState";
import {
  getClaimDescriptionRowsFromRows,
  readClaimRows,
} from "~/modules/bitcraft/gamestate/claimDescription";

export interface ExtendedExpeirenceStateRowWithName
  extends ExtendedExpeirenceStateRow {
  entity_name?: string;
}

export default defineEventHandler((event) => {
  const claimsRows = getClaimDescriptionRowsFromRows(readClaimRows());
  const id = getRouterParam(event, "id", { decode: true });

  if (!id) {
    throw createError({
      statusCode: 400,
      statusMessage: "Missing ID",
    });
  }

  const claims = claimsRows.find((claims) => claims.entity_id == parseInt(id));

  if (!claims) {
    throw createError({
      statusCode: 404,
      statusMessage: "Claim was not found",
    });
  }

  const players: number[] = claims.members.map((member) => member.entity_id);

  const response = {};

  const leaderboardState = getLeaderboard();

  for (const key in leaderboardState) {
    response[key] = leaderboardState[key].filter((row) => {
      return players.includes(row.player_id);
    });
  }

  return {
    ...response,
  };
});
