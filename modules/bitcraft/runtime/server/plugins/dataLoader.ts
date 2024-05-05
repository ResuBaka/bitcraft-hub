import {parseInventorys, readInventoryRows, saveParsedInventorys} from "~/modules/bitcraft/gamestate/inventory";

export default defineNitroPlugin(async (nitroApp) => {
    const inventoryRows =  readInventoryRows();
    const parsedInventoryRows = parseInventorys(inventoryRows);
    saveParsedInventorys(parsedInventoryRows);
});
