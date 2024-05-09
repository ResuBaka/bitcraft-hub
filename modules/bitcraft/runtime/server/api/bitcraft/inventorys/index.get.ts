import {
  getInventorys,
  replaceInventoryItemsIdWithItems,
} from "~/modules/bitcraft/gamestate/inventory";
import {
  getItemRowsFromRows,
  readItemRows,
} from "~/modules/bitcraft/gamestate/item";

const items = getItemRowsFromRows(readItemRows());
const rows = replaceInventoryItemsIdWithItems(getInventorys(), items);

let perPageDefault = 24;
let perPageMax = perPageDefault * 4;

export default defineEventHandler((event) => {
  let { search, page, owner_entity_id, perPage } = getQuery(event);

  if (owner_entity_id) {
    owner_entity_id = parseInt(owner_entity_id);
  }

  if (page) {
    page = parseInt(page);
  } else {
    page = 1;
  }

  if (perPage) {
    perPage = parseInt(perPage);
    if (perPage > perPageMax) {
      perPage = perPageDefault;
    }
  } else {
    perPage = perPageDefault;
  }

  const rowsFilterted =
    rows?.filter((building: any) => {
      return (
        (!owner_entity_id || building.owner_entity_id === owner_entity_id) &&
        (!search ||
          building.name.toLowerCase().includes(search.toLowerCase()) ||
          !search ||
          building.entity_id.toString().includes(search))
      );
    }) ?? [];

  return {
    inventorys: rowsFilterted.slice((page - 1) * perPage, page * perPage),
    total: rowsFilterted.length,
    page,
    perPage,
  };
});
