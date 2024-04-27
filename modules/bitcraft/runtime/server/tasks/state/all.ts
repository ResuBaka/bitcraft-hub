import { readFile, writeFile } from "node:fs/promises";
import SQLRequest from "./../../../SQLRequest";
let rootFolder = `${process.cwd()}/storage/State`;
let allDescTables = [  "ActiveBuffState",
"LocationState",
]
export default defineTask({
    meta: {
        name: "fetch:all:state",
        description: "Run database migrations",
    },
    async run({ payload, context }) {
        for (var descTable of allDescTables) {
            const sql = `SELECT * FROM ${descTable}`
            const result = await SQLRequest<any>(sql)
            await writeFile(`${rootFolder}/${descTable}.json`, JSON.stringify(result))
        }

        return { result: "Success" };
    },
});
//640