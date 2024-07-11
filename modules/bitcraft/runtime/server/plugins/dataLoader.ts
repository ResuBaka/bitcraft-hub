import {
  parseInventorys,
  readInventoryRows,
  reloadInventoryState,
  saveParsedInventorys,
} from "~/modules/bitcraft/gamestate/inventory";

export default defineNitroPlugin(async (nitroApp) => {
  // await Promise.allSettled([
  //     runTask("desc:all"),
  //     runTask("state:all"),
  // ]);

  reloadInventoryState();
});
