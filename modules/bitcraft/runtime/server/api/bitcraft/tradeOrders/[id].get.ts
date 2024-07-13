import {
  getCargoDescRowsFromRows,
  readCargoDescRows,
} from "~/modules/bitcraft/gamestate/cargoDesc";
import {
  getItemRowsFromRows,
  readItemRows,
} from "~/modules/bitcraft/gamestate/item";
import {
  getTradingOrderStateRowsFromRows,
  readTradeOrderStateRows,
  replaceTradeOrderCargoIdWithCargo,
  replaceTradeOrderItemIdWithItem,
} from "~/modules/bitcraft/gamestate/tradeOrder";

const items = getItemRowsFromRows();
const cargo_rows = getCargoDescRowsFromRows(readCargoDescRows());
const rows = getTradingOrderStateRowsFromRows(readTradeOrderStateRows());
export default defineEventHandler((event) => {
  const id = getRouterParam(event, "id", { decode: true });

  if (!id) {
    throw createError({
      statusCode: 400,
      statusMessage: "Missing ID",
    });
  }

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
