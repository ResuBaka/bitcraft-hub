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
import { SqlRequestInventoryByEntityId } from "../gamestate/inventory";
import { getEquipmentRowsFromRows, readEquipmentRows } from "../gamestate/equipment";
import { getTradingOrderStateRowsFromRows, readTradeOrderStateRows } from "../gamestate/tradeOrder";
let usernames = ["Ryuko"];

export default async function RequestAllPlayerInfo() {
  const equipments = getTradingOrderStateRowsFromRows(readTradeOrderStateRows())
}
RequestAllPlayerInfo();
