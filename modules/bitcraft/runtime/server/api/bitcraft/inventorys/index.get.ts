import {
  getInventorys,
  type InventoryStateRow,
  replaceInventoryItemsIdWithItems,
} from "~/modules/bitcraft/gamestate/inventory";
import { getItemRowsFromRows } from "~/modules/bitcraft/gamestate/item";
import { getPlayerRowsFromRows } from "~/modules/bitcraft/gamestate/player";
import {
  getVehicleState,
  VehicleStateRow,
} from "~/modules/bitcraft/gamestate/vehicleState";
import {
  getCagoDescFromCargoId,
  getCargoDescRowsFromRows,
} from "~/modules/bitcraft/gamestate/cargoDesc";

let perPageDefault = 24;
let perPageMax = perPageDefault * 6;

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
  const items = getItemRowsFromRows();
  const rows = getInventorys();
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

  const player = getPlayerRowsFromRows().find(
    (player) => player.entity_id == owner_entity_id,
  );
  let searchPersonalStorageChest = undefined;
  let vehicleStateRows: VehicleStateRow[] = [];

  if (player) {
    vehicleStateRows = getVehicleState().filter(
      (vehicle) => vehicle.owner_id == owner_entity_id,
    );

    if (vehicleStateRows.length) {
      searchPersonalStorageChest = vehicleStateRows.map(
        (vehicle) => vehicle.entity_id,
      );
    }
  }

  const needsToFilter = !!owner_entity_id || !!search;

  const rowsFilterted = needsToFilter
    ? (rows?.filter((inventory) => {
        let found = false;

        if (owner_entity_id && inventory.owner_entity_id === owner_entity_id) {
          return true;
        }

        if (
          searchPersonalStorageChest &&
          searchPersonalStorageChest.includes(inventory.owner_entity_id)
        ) {
          return true;
        }

        if (search && inventory.entity_id.toString().includes(search)) {
          return true;
        }

        return found;
      }) ?? [])
    : rows;

  const inventorys = Array.of(
    ...replaceInventoryItemsIdWithItems(
      rowsFilterted.slice((page - 1) * perPage, page * perPage),
      items,
    ),
  ).sort((a, b) => a.entity_id - b.entity_id);

  const cargo_rows = getCargoDescRowsFromRows();

  for (let i = 0; i < inventorys.length; i++) {
    let inventory = inventorys[i];

    if (owner_entity_id && (i === 0 || i === 1)) {
      inventorys[i].nickname = i == 0 ? "Tool belt" : "Inventory";
    }

    if (
      searchPersonalStorageChest &&
      searchPersonalStorageChest.includes(inventorys[i].owner_entity_id)
    ) {
      for (let vehicleStateRow of vehicleStateRows) {
        if (vehicleStateRow.entity_id == inventorys[i].owner_entity_id) {
          inventorys[i].nickname = vehicleStateRow.nickname;
        }
      }
    }

    for (
      let pocket_index = 0;
      pocket_index < inventory.pockets.length;
      pocket_index++
    ) {
      let pocket = inventory.pockets[pocket_index];
      if (pocket.contents !== undefined) {
        if (pocket.contents.item_type === "Cargo") {
          inventorys[i].pockets[pocket_index].contents = {
            ...pocket.contents,
            item: getCagoDescFromCargoId(cargo_rows, pocket.contents.item_id),
          };
        }
      }
    }
  }

  return {
    inventorys,
    total: rowsFilterted.length,
    page,
    perPage,
  };
});
