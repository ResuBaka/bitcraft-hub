import {
  getInventoryRowsFromRows,
  readInventroyChanges,
} from "~/modules/bitcraft/gamestate/inventory";

export default defineEventHandler((event) => {
  const id = getRouterParam(event, "id", { decode: true });

  if (!id) {
    throw createError({
      statusCode: 400,
      statusMessage: "Missing ID",
    });
  }

  const data = readInventroyChanges(parseInt(id));
  if (!data) {
    throw createError({
      statusCode: 404,
      statusMessage: "InventoryChanged was not found",
    });
  }

  return data;
});
