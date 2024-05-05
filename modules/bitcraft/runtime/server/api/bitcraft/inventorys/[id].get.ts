import {
  getInventorys,
  replaceInventoryItemIdWithItem,
} from "~/modules/bitcraft/gamestate/inventory";

export default defineEventHandler((event) => {
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
      statusMessage: "Claim was not found",
    });
  }

  return replaceInventoryItemIdWithItem(claims);
});
