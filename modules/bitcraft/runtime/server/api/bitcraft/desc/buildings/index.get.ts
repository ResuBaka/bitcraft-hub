import {
  type BuildingDescRow,
  getBuildingDescRowsFromRows,
} from "~/modules/bitcraft/gamestate/buildingDesc";
import type { BuildingStateRow } from "~/modules/bitcraft/gamestate/buildingState";
import { getBuildingStateRowsFromRows } from "~/modules/bitcraft/gamestate/buildingState";

export default defineEventHandler((event) => {
  let { search, page, per_page } = getQuery(event);

  let rows = getBuildingDescRowsFromRows();

  if (page) {
    page = parseInt(page);
  } else {
    page = 1;
  }

  if (per_page) {
    per_page = parseInt(per_page);
  } else {
    per_page = 1;
  }

  const rowsFilterted =
    rows?.filter(
      (building: any) =>
        !search ||
        building.name.toLowerCase().includes(search.toLowerCase()) ||
        !search ||
        building.id.toString().includes(search),
    ) ?? [];

  const buildingStateRows = getBuildingStateRowsFromRows();

  for (const building of rowsFilterted) {
    addCountOfBuildingsInWorldForBuilding(building, buildingStateRows); // TODO:
  }

  return {
    buildings: rowsFilterted.slice((page - 1) * per_page, page * per_page),
    total: rowsFilterted.length,
    page,
    per_page,
  };
});

function addCountOfBuildingsInWorldForBuilding(
  building: BuildingDescRow,
  buildingStates: BuildingStateRow[],
): BuildingDescRow {
  building.count = buildingStates.filter(
    (buildingState) => buildingState.building_description_id === building.id,
  ).length;
  return building;
}
