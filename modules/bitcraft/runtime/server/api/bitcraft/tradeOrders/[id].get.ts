import { getCargoDescRowsFromRows } from "~/modules/bitcraft/gamestate/cargoDesc";
import { getItemRowsFromRows } from "~/modules/bitcraft/gamestate/item";
import {
  getTradingOrderStateRowsFromRows,
  replaceTradeOrderCargoIdWithCargo,
  replaceTradeOrderItemIdWithItem,
} from "~/modules/bitcraft/gamestate/tradeOrder";

export default defineEventHandler((event) => {
  const id = getRouterParam(event, "id", { decode: true });

  if (!id) {
    throw createError({
      statusCode: 400,
      statusMessage: "Missing ID",
    });
  }

  const items = getItemRowsFromRows();
  const cargo_rows = getCargoDescRowsFromRows();
  const rows = getTradingOrderStateRowsFromRows();
  const claims = rows.find((claims) => claims.entity_id == parseInt(id));

  if (!claims) {
    throw createError({
      statusCode: 404,
      statusMessage: "Claim was not found",
    });
  }

  return replaceTradeOrderItemIdWithItem(
    replaceTradeOrderCargoIdWithCargo(claims, cargo_rows),
    items,
  );
});
