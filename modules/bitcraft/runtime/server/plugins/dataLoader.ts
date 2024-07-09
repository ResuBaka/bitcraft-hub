import {
  parseInventorys,
  readInventoryRows,
  saveParsedInventorys,
} from "~/modules/bitcraft/gamestate/inventory";

export default defineNitroPlugin(async (nitroApp) => {
  // await Promise.allSettled([
  //     runTask("desc:all"),
  //     runTask("state:all"),
  // ]);

  const inventoryRows = readInventoryRows();
  const parsedInventoryRows = parseInventorys(inventoryRows);
  saveParsedInventorys(parsedInventoryRows);
});
