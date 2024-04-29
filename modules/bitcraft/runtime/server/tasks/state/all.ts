import { readFile, writeFile } from "node:fs/promises";
import { createWriteStream } from "node:fs";
import { finished } from "node:stream/promises";
import { Readable } from "node:stream";
import {SQLRequestStream} from "./../../../SQLRequest";
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
            const result = await SQLRequestStream(sql)
            const stream = createWriteStream(`${rootFolder}/${descTable}.json`)
            await finished(Readable.fromWeb(result).pipe(stream))
        }

        return { result: "Success" };
    },
});
//640
