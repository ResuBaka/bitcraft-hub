import {
  getInventorys,
  replaceInventoryItemIdWithItem,
} from "~/modules/bitcraft/gamestate/inventory";
import { getItemRowsFromRows } from "~/modules/bitcraft/gamestate/item";

export default defineEventHandler((event) => {
  const items = getItemRowsFromRows();
  const rows = getInventorys();

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
      statusMessage: "Inventory was not found",
    });
  }

  return replaceInventoryItemIdWithItem(claims, items);
});
