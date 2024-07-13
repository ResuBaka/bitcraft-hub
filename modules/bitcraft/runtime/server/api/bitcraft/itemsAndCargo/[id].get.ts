import {
  getItemRowsFromRows,
  type ItemRow,
} from "~/modules/bitcraft/gamestate/item";

import {
  type CargoDescRow,
  getCargoDescRowsFromRows,
} from "~/modules/bitcraft/gamestate/cargoDesc";

export default defineEventHandler((event) => {
  const id = getRouterParam(event, "id", { decode: true });

  if (!id) {
    throw createError({
      statusCode: 400,
      statusMessage: "Missing ID",
    });
  }

  const itemRows = getItemRowsFromRows();

  let item: ItemRow | undefined | CargoDescRow = itemRows.find(
    (item) => item.id == parseInt(id),
  );

  if (!item) {
    const cargoDescRows = getCargoDescRowsFromRows();
    item = cargoDescRows.find((item) => item.id == parseInt(id));
  }

  if (!item) {
    throw createError({
      statusCode: 404,
      statusMessage: "Item was not found",
    });
  }

  return item;
});
