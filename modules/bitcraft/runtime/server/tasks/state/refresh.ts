import fs, { createWriteStream } from "node:fs";
import SQLRequest, { SQLRequestStream } from "./../../../SQLRequest";
import { rebuildLeaderboardState } from "../../../../gamestate/experienceState";
import { writeFile } from "node:fs/promises";
import { reloadInventoryState } from "../../../../gamestate/inventory";
import { reloadBuildingState } from "../../../../gamestate/buildingState";
import { reloadClaimDescription } from "../../../../gamestate/claimDescription";
import { reloadTradingOrderState } from "../../../../gamestate/tradeOrder";
import { reloadVehicleState } from "../../../../gamestate/vehicleState";
import { reloadPlayerState } from "../../../../gamestate/player";
import { reloadClaimTechState } from "../../../../gamestate/claimTechState";
let rootFolder = `${process.cwd()}/storage/State`;
let allDescTables = [
  "PlayerState",
  "ExperienceState",
  "ClaimDescriptionState",
  "ClaimRecruitmentState",
  "ClaimTechState",
  "ClaimTileState",
  "TradeOrderState",
  "InventoryState",
  "BuildingState",
  "VehicleState",
  "ClaimTechState",
  "MobileEntityState",
];

export default defineTask({
  meta: {
    name: "fetch:all:state",
    description: "Run database migrations",
  },
  async run({ payload, context }) {
    const config = useRuntimeConfig().bitcraft;
    if (config.disable.refresh) {
      return { result: "Refresh disabled" };
    }

    for (var descTable of allDescTables) {
      try {
        console.log(descTable);
        const sql = `SELECT * FROM ${descTable}`;

        const filePath = `${rootFolder}/${descTable}.json`;
        if (fs.existsSync(filePath)) {
          fs.unlinkSync(filePath);
          console.log("Deleted file", filePath);
        }

        const result = await SQLRequest<any>(sql);
        await writeFile(filePath, JSON.stringify(result));

        // const result = await SQLRequestStream(sql);
        // const stream = createWriteStream(filePath);
        // console.log("Writing to file", filePath);
        // await finished(Readable.fromWeb(result).pipe(stream));
      } catch (e) {
        console.error(e);
      }
    }

    rebuildLeaderboardState();
    reloadInventoryState();
    reloadBuildingState();
    reloadClaimDescription();
    reloadTradingOrderState();
    reloadVehicleState();
    reloadPlayerState();
    reloadClaimTechState();

    return { result: "Success" };
  },
});
