import {
  getInventorys,
  type InventoryStateRow,
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

export type InventoryQuery = {
  search?: string;
  page?: number;
  owner_entity_id?: number;
  perPage?: number;
};

export type InventoryResponse = {
  inventorys: InventoryStateRow[];
  total: number;
  page: number;
  perPage: number;
};

export default defineEventHandler<InventoryResponse>((event) => {
  let { search, page, owner_entity_id, perPage } =
    getQuery<InventoryQuery>(event);

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
    rows?.filter((inventory) => {
      return (
        (!owner_entity_id || inventory.owner_entity_id === owner_entity_id) &&
        (!search || inventory.entity_id.toString().includes(search))
      );
    }) ?? [];

  return {
    inventorys: rowsFilterted.slice((page - 1) * perPage, page * perPage),
    total: rowsFilterted.length,
    page,
    perPage,
  };
});
