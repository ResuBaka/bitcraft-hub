import { writeFile } from "node:fs/promises";
import SQLRequest from "./../../../SQLRequest";
let rootFolder = `${process.cwd()}/storage/Rest`;
let allDescTables = [
  "AdminBroadcast",
  "ChatCache",
  "ClaimTileCost",
  "Config",
  "Globals",
  "GlobalsAppeared",
  "HerdCache",
  "IdentityRole",
  "LocationCache",
  "ResourceCount",
  "ResourcesLog",
  "RestBuffIds",
  "ServerIdentity",
];

export default defineTask({
  meta: {
    name: "fetch:all:state",
    description: "Run database migrations",
  },
  async run({ payload, context }) {
    for (var descTable of allDescTables) {
      const sql = `SELECT * FROM ${descTable}`;
      const result = await SQLRequest<any>(sql);
      await writeFile(
        `${rootFolder}/${descTable}.json`,
        JSON.stringify(result),
      );
    }

    return { result: "Success" };
  },
});
