import { getBuildingDescIdMapFromRows } from "~/modules/bitcraft/gamestate/buildingDesc";
import { getBuildingStateRowsFromRows } from "~/modules/bitcraft/gamestate/buildingState";

export default defineEventHandler((event) => {
  let { search, with_inventory, page, claim_entity_id } = getQuery(event);

  let rows = getBuildingStateRowsFromRows();

  const perPage = 30;
  if (claim_entity_id) {
    claim_entity_id = parseInt(claim_entity_id);
  }
  if (with_inventory) {
    with_inventory = Boolean(with_inventory);
  }

  if (page) {
    page = parseInt(page);
  } else {
    page = 1;
  }
  if (with_inventory == true) {
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
  }

  const rowsFilterted =
    rows?.filter((building: any) => {
      return (
        (!claim_entity_id || building.claim_entity_id === claim_entity_id) &&
        (!search ||
          building.name.toLowerCase().includes(search.toLowerCase()) ||
          !search ||
          building.entity_id.toString().includes(search))
      );
    }) ?? [];

  return {
    buildings: rowsFilterted.slice((page - 1) * perPage, page * perPage),
    total: rowsFilterted.length,
    page,
    perPage,
  };
});
