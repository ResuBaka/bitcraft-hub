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
import {
  getEquipmentRowsFromRows,
  readEquipmentRows,
} from "../gamestate/equipment";
import {
  getTradingOrderStateRowsFromRows,
  readTradeOrderStateRows,
  replaceTradeOrderCargoIdWithCargo,
  replaceTradeOrdersCargoIdWithCargo,
} from "../gamestate/tradeOrder";
import { readCargoStateRows } from "../gamestate/cargoState";
import {
  getCargoDescRowsFromRows,
  readCargoDescRows,
} from "../gamestate/cargoDesc";
import { getCraftingRecipesFromRows, readCraftingRecipeRows } from "../gamestate/rerecipe";
import { getItemListRowsFromRows, readItemListRows } from "../gamestate/itemListDesc";
let usernames = ["Ryuko"];

export default async function RequestAllPlayerInfo() {
  const rows = getItemListRowsFromRows(readItemListRows())
  console.log(JSON.stringify(rows.filter((row) => row.item_list_id !== 0)))
}

RequestAllPlayerInfo();
