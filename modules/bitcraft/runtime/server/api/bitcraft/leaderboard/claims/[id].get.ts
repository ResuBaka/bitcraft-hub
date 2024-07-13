import { getLeaderboard } from "~/modules/bitcraft/gamestate/experienceState";
import { getClaimDescriptionRowsFromRows } from "~/modules/bitcraft/gamestate/claimDescription";

export default defineEventHandler((event) => {
  const id = getRouterParam(event, "id", { decode: true });

  if (!id) {
    throw createError({
      statusCode: 400,
      statusMessage: "Missing ID",
    });
  }

  const claimsRows = getClaimDescriptionRowsFromRows();
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
