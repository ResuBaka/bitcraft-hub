import {
  ClaimDescriptionRow,
  getClaimDescriptionRowsFromRows,
  readClaimRows,
} from "~/modules/bitcraft/gamestate/claimDescription";
import type { BuildingStateRow } from "~/modules/bitcraft/gamestate/buildingState";
import {
  getBuildingStateRowsFromRows,
  readBuildingStateRows,
} from "~/modules/bitcraft/gamestate/buildingState";
import {
  getBuildingDescIdMapFromRows,
  readBuildingDescRows,
} from "~/modules/bitcraft/gamestate/buildingDesc";
import {
  getInventorys,
  replaceInventoryItemsIdWithItems,
} from "~/modules/bitcraft/gamestate/inventory";
import {
  getItemRowsFromRows,
  readItemRows,
} from "~/modules/bitcraft/gamestate/item";

const rows = getClaimDescriptionRowsFromRows(readClaimRows());
export default defineEventHandler((event) => {
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
  let rows = getBuildingStateRowsFromRows(readBuildingStateRows());

  const buildingDescMap = getBuildingDescIdMapFromRows(readBuildingDescRows());
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

  console.log("found buidlings", rowsFilterted.length);

  const buildingIds = rowsFilterted.map((building) => building.entity_id);
  const itemsTemp = getItemRowsFromRows(readItemRows());

  const rowsINventory = replaceInventoryItemsIdWithItems(
    getInventorys(),
    itemsTemp,
  );

  const rowsFiltertedINventory =
    rowsINventory?.filter((inventory) => {
      return buildingIds.includes(inventory.owner_entity_id);
    }) ?? [];

  let items = {};

  for (const inventory of rowsFiltertedINventory) {
    console.log(inventory);

    for (const pocket of inventory.pockets) {
      if (pocket.contents !== undefined) {
        if (pocket.contents.item_type === "Cargo") {
          continue;
        }

        if (items[pocket.contents.item_id] === undefined) {
          items[pocket.contents.item_id] = pocket.contents;
        } else {
          items[pocket.contents.item_id].quantity += pocket.contents.quantity;
        }
      }
    }
  }

  return items;
}
