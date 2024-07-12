import {
  getEmpireStateRowsFromRows,
  readEmpireState,
} from "~/modules/bitcraft/gamestate/empireState";

import {
  getEmpireRankStateRowsFromRows,
  readEmpireRankState,
} from "~/modules/bitcraft/gamestate/empireRankState";


import {
  getEmpirePlayerDataStateRowsFromRows,
  readEmpirePlayerDataState,
} from "~/modules/bitcraft/gamestate/empirePlayerDataState";

import {
  getEmpireSettlementStateRowsFromRows,
  readEmpireSettlementState,
} from "~/modules/bitcraft/gamestate/empireSettlementState";

export default defineEventHandler((event) => {
  const id = getRouterParam(event, "id", { decode: true });

  const rows = getEmpireStateRowsFromRows(readEmpireState());
  const rankRows = getEmpireRankStateRowsFromRows(readEmpireRankState());
  const playerRows = getEmpirePlayerDataStateRowsFromRows(readEmpirePlayerDataState());
  const claimRows = getEmpireSettlementStateRowsFromRows(readEmpireSettlementState());
  if (!id) {
    throw createError({
      statusCode: 400,
      statusMessage: "Missing ID",
    });
  }

  const empire = rows.find((claims) => claims.entity_id == parseInt(id));
  const ranks = rankRows.filter((claims) => claims.empire_entity_id == parseInt(id));
  const players = playerRows.filter((claims) => claims.empire_entity_id == parseInt(id));
  const claims = playerRows.filter((claims) => claims.empire_entity_id == parseInt(id));

  if (!empire) {
    throw createError({
      statusCode: 404,
      statusMessage: "Empire was not found",
    });
  }

  return {
    ...empire,
    players,
    ranks,
    claims
  };
});
