import fs, { createWriteStream } from "node:fs";
import SQLRequest, { SQLRequestStream } from "./../../../SQLRequest";
import { rebuildLeaderboardState } from "../../../../gamestate/experienceState";
import { writeFile } from "node:fs/promises";
import { reloadInventoryState } from "../../../../gamestate/inventory";
import { reloadBuildingState } from "../../../../gamestate/buildingState";
import { reloadClaimDescription } from "../../../../gamestate/claimDescription";
import { reloadTradingOrderState } from "../../../../gamestate/tradeOrder";
import { reloadVehicleState } from "../../../../gamestate/vehicleState";
let rootFolder = `${process.cwd()}/storage/State`;
let allDescTables = [
  "PlayerState",
  "ExperienceState",
  "ClaimDescriptionState",
  "ClaimRecruitmentState",
  "ClaimTechState",
  "ClaimTileState",
  "TradeOrderState",
  "BuildingState",
  "VehicleState",
];

export default defineTask({
  meta: {
    name: "fetch:all:state",
    description: "Run database migrations",
  },
  async run({ payload, context }) {
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
    reloadBuildingState();
    reloadClaimDescription();
    reloadTradingOrderState();
    reloadVehicleState();

    return { result: "Success" };
  },
});
//640
