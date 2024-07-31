import { readInventroyChanges } from "~/modules/bitcraft/gamestate/inventory";
import { getItemRowsFromRows } from "~/modules/bitcraft/gamestate/item";
import { getCargoDescRowsFromRows } from "~/modules/bitcraft/gamestate/cargoDesc";

export default defineEventHandler((event) => {
  const id = getRouterParam(event, "id", { decode: true });

  if (!id) {
    throw createError({
      statusCode: 400,
      statusMessage: "Missing ID",
    });
  }

  let items = getItemRowsFromRows();

  let item_map = new Map<number, any>();

  for (const item of items) {
    item_map.set(item.id, item);
  }

  let cargos = getCargoDescRowsFromRows();
  let cargo_map = new Map<number, any>();

  for (const cargo of cargos) {
    cargo_map.set(cargo.id, cargo);
  }

  const data = readInventroyChanges(parseInt(id));
  if (!data) {
    throw createError({
      statusCode: 404,
      statusMessage: "InventoryChanged was not found",
    });
  }

  for (let i = 0; i < data.length; i++) {
    if (data[i].diff === undefined) {
      continue;
    }

    for (const diff of Object.keys(data[i].diff)) {
      if (data[i].diff[diff].old) {
        if (data[i].diff[diff].old.item_type === "Cargo") {
          data[i].diff[diff].old.item = cargo_map.get(
            data[i].diff[diff].old.item_id,
          );
        }

        if (data[i].diff[diff].old.item_type === "Item") {
          data[i].diff[diff].old.item = item_map.get(
            data[i].diff[diff].old.item_id,
          );
        }
      }

      if (data[i].diff[diff].new) {
        if (data[i].diff[diff].new.item_type === "Cargo") {
          data[i].diff[diff].new.item = cargo_map.get(
            data[i].diff[diff].new.item_id,
          );
        }

        if (data[i].diff[diff].new.item_type === "Item") {
          data[i].diff[diff].new.item = item_map.get(
            data[i].diff[diff].new.item_id,
          );
        }
      }
    }
  }

  return data.reverse();
});
