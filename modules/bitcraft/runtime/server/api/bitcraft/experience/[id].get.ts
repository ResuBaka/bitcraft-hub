import { getLeaderboard } from "~/modules/bitcraft/gamestate/experienceState";

export default defineEventHandler((event) => {
  let id = getRouterParam(event, "id", { decode: true });

  if (!id) {
    throw createError({
      statusCode: 400,
      statusMessage: "Missing ID",
    });
  }

  id = parseInt(id);

  if (isNaN(id)) {
    throw createError({
      statusCode: 400,
      statusMessage: "Invalid ID",
    });
  }

  const leaderboard = getLeaderboard();
  const response = {};

  for (const skill of Object.keys(leaderboard)) {
    const leaders = leaderboard[skill];

    const row = leaders.find((row) => row.player_id === id);
    if (row) {
      response[skill] = row;
    }
  }

  if (!Object.keys(response).length) {
    throw createError({
      statusCode: 404,
      statusMessage: "Claim was not found",
    });
  }

  return {
    ...response,
  };
});
