import {
  getCargoDescRowsFromRows,
  readCargoDescRows,
} from "~/modules/bitcraft/gamestate/cargoDesc";

const rows = getCargoDescRowsFromRows(readCargoDescRows());
export default defineEventHandler((event) => {
  const id = getRouterParam(event, "id", { decode: true });

  if (!id) {
    throw createError({
      statusCode: 400,
      statusMessage: "Missing ID",
    });
  }

  const item = rows.find((item) => item.id == parseInt(id));

  if (!item) {
    throw createError({
      statusCode: 404,
      statusMessage: "Item was not found",
    });
  }

  return item;
});
