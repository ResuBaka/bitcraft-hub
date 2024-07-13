import {
  type ClaimDescriptionRow,
  getClaimDescriptionRowsFromRows,
} from "~/modules/bitcraft/gamestate/claimDescription";
import { getBuildingStateRowsFromRows } from "~/modules/bitcraft/gamestate/buildingState";
import { getBuildingDescIdMapFromRows } from "~/modules/bitcraft/gamestate/buildingDesc";
import {
  getInventorys,
  replaceInventoryItemIdWithItem,
  replaceInventoryItemsIdWithItems,
} from "~/modules/bitcraft/gamestate/inventory";
import { getItemRowsFromRows } from "~/modules/bitcraft/gamestate/item";
import {
  getCagoDescFromCargoId,
  getCargoDescRowsFromRows,
} from "~/modules/bitcraft/gamestate/cargoDesc";

export default defineEventHandler((event) => {
  const rows = getClaimDescriptionRowsFromRows();
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

  return {
    ...claims,
    inventorys: getInventorysFromClaimMerged(claims),
  };
});

function getInventorysFromClaimMerged(claim: ClaimDescriptionRow) {
  let rows = getBuildingStateRowsFromRows();

  const buildingDescMap = getBuildingDescIdMapFromRows();
  rows = rows.filter((buildingState) => {
    const buildingDesc = buildingDescMap.get(
      buildingState.building_description_id,
    );

    buildingState.building_name = buildingDesc?.name;
    buildingState.image_path = buildingDesc?.icon_asset_name + ".png";

    if (buildingDesc === undefined) {
      return false;
    }
    if (buildingDesc.name.includes("Chest")) {
      return true;
    }
    if (buildingDesc.name.includes("Stockpile")) {
      return true;
    }
    return false;
  });

  const rowsFilterted =
    rows?.filter((building: any) => {
      return building.claim_entity_id === claim.entity_id;
    }) ?? [];

  const buildingIds = rowsFilterted.map((building) => building.entity_id);
  const itemsTemp = getItemRowsFromRows();
  const cargo_rows = getCargoDescRowsFromRows();

  const rowsINventory = replaceInventoryItemsIdWithItems(
    getInventorys().filter((inventory) =>
      buildingIds.includes(inventory.owner_entity_id),
    ) ?? [],
    itemsTemp,
  );

  let items = {};

  for (const inventory of rowsINventory) {
    for (const pocket of inventory.pockets) {
      if (pocket.contents !== undefined) {
        if (
          pocket.contents.item_type === "Cargo" &&
          items[pocket.contents.item_id] === undefined
        ) {
          items[pocket.contents.item_id] = {
            ...pocket.contents,
            item: getCagoDescFromCargoId(cargo_rows, pocket.contents.item_id),
          };

          continue;
        } else if (pocket.contents.item_type === "Cargo") {
          items[pocket.contents.item_id].quantity += pocket.contents.quantity;
          continue;
        }

        if (
          pocket.contents.item_type === "Item" &&
          items[pocket.contents.item_id] === undefined
        ) {
          items[pocket.contents.item_id] = { ...pocket.contents };
          continue;
        } else if (pocket.contents.item_type === "Item") {
          items[pocket.contents.item_id].quantity += pocket.contents.quantity;
          continue;
        }
      }
    }
  }

  return Object.values(items).sort((a, b) =>
    a.quantity > b.quantity ? -1 : 1,
  );
}
