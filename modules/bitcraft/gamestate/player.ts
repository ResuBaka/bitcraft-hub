import { readFile, writeFile } from "node:fs/promises";
import SQLRequest from "../runtime/SQLRequest";
import {getItemRowsFromRows, readItemRows } from "../gamestate/item"
let usernames = [
    "Sweets"
]

export async function loadFile(file: any) {
    const fileData = await readFile(file);
  
    return JSON.parse(await readFile(fileData, "utf-8"));
  }
type PlayerStateRow = {
    entity_id: Number,
    serial_id: Number,
    username: string
    eth_pub_key: string,
    time_played: Number,
    session_start_timestamp: Number
    time_signed_in: Number
    sign_in_timestamp: Number
    signed_in: boolean
    unmanned_vehicle_coords: any
    destination_marker: any
    favorite_crafting_recipes: any
    teleport_location: any
}
export function getPlayerRowsFromRows(rows: any[][]) {
    console.log("getItemRowsFromRows")
    const itemRows: PlayerStateRow[] = []
    for (const row of rows) {
        console.log("working")
        itemRows.push(getPlayerRowFromRow(row))
    }


    return itemRows
}

function getPlayerRowFromRow(row: any[]){
    const PlayerState: PlayerStateRow = {
        entity_id: row[0] as unknown as number,
        serial_id: row[1] as unknown as number,
        username: row[2],
        eth_pub_key: row[3],
        time_played: row[4] as unknown as number,
        session_start_timestamp: row[5] as unknown as number,
        time_signed_in: row[6] as unknown as number,
        sign_in_timestamp: row[7] as unknown as number,
        signed_in: row[8] as unknown as boolean,
        unmanned_vehicle_coords: row[9],
        destination_marker: row[10],
        favorite_crafting_recipes: row[11],
        teleport_location: row[12]
    }
    return PlayerState
}

export function SqlRequestPlayers() {
    let sql = ""
    for(const username of usernames){
        if(sql.length === 0){
            sql = `SELECT * FROM PlayerState WHERE username = '${username}'`
        }else{
            sql + ` or username = '${username}'`
        }
    }
    return SQLRequest<any>(sql)
}