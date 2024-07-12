import fs, { createWriteStream } from "node:fs";
import { finished } from "node:stream/promises";
import { Readable } from "node:stream";
import SQLRequest, { SQLRequestStream } from "./../../../SQLRequest";
import { rebuildLeaderboardState } from "../../../../gamestate/experienceState";
import { writeFile } from "node:fs/promises";
import {
  parseInventorys,
  readInventoryRows,
  reloadInventoryState,
  saveParsedInventorys,
} from "../../../../gamestate/inventory";
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
  "EmpireState",
  "EmpireSettlementState"

];
export default defineTask({
  meta: {
    name: "refetch:all:state",
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

    console.log("Rebuilding Leaderboard");
    rebuildLeaderboardState();
    console.log("Rebuilding Leaderboard Complete");

    console.log("Reloading Inventorys");
    reloadInventoryState();
    console.log("Reloading Inventorys Complete");

    return { result: "Success" };
  },
});
//640
