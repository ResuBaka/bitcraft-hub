import {
  getPlayerRowsFromRows,
  SqlRequestPlayersByUsername,
} from "../gamestate/player";
import {
  getClaimDescriptionRowsFromRows,
  SqlRequestClaimDescriptionByPlayerEntityId,
} from "../gamestate/claimDescription";
import {
  getBuildingStateRowsFromRows,
  SqlRequesttBuildingStateByClaimEntityId,
} from "../gamestate/buildingState";
import {
  getBuildingDescIdMapFromRows,
  readBuildingDescRows,
} from "../gamestate/buildingDesc";
import {
  SqlRequestInventoryByEntityId,
} from "../gamestate/inventory";
let usernames = ["Ryuko"];

export default async function RequestAllPlayerInfo() {
  const players = getPlayerRowsFromRows(
    await SqlRequestPlayersByUsername(usernames),
  );

  const claim = getClaimDescriptionRowsFromRows(
    await SqlRequestClaimDescriptionByPlayerEntityId(players),
  );
  const buildingState = getBuildingStateRowsFromRows(
    await SqlRequesttBuildingStateByClaimEntityId(claim),
  );
  const buildingDescMap = getBuildingDescIdMapFromRows(readBuildingDescRows());
  const chests = buildingState.filter((buildingState) => {
    const buildingDesc = buildingDescMap.get(
      buildingState.building_description_id,
    );
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
  const data = await SqlRequestInventoryByEntityId(chests);
}
RequestAllPlayerInfo();
