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
import { getAllConsumedItemsFromItem, getCraftingRecipesFromRows, readCraftingRecipeRows } from "../gamestate/rerecipe";
import { getItemListRowsFromRows, readItemListRows } from "../gamestate/itemListDesc";
import { getItemRowsFromRows, readItemRows } from "../gamestate/item";
import fs from "node:fs"
let usernames = ["Ryuko"];

const rows = getCraftingRecipesFromRows(readCraftingRecipeRows());
const items = getItemRowsFromRows(readItemRows());
const item_list = getItemListRowsFromRows(readItemListRows())


export default async function RequestAllPlayerInfo() {
  console.time("add");
  const abc = getAllConsumedItemsFromItem(rows, 1050001,items,item_list);
  console.timeEnd("add");
  const data = JSON.stringify(abc)
  fs.writeFile ("input.json", data, function(err) {
    if (err) throw err;
    console.log('complete');
    }
);
console.log(data.length)
}

RequestAllPlayerInfo();
