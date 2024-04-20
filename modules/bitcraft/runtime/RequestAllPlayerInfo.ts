import { readFile, writeFile } from "node:fs/promises";
import SQLRequest from "./SQLRequest";
let rootFolder = `${process.cwd()}/storage/Desc`;
let usernames = [
    "Boegie19"
]

type PlayerState = {
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
function transformPlayerState(input){
    let PlayerStateArray: Array<PlayerState> = []
    for(const row of input[0].rows){
        const PlayerState: PlayerState = {
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
        PlayerStateArray.push(PlayerState)
    }
    return PlayerStateArray
}
export default async function RequestAllPlayerInfo() {
        let sql
        for(const username of usernames){
            if(sql === undefined){
                sql = `SELECT * FROM PlayerState WHERE username = '${username}'`
            }else{
                sql + ` or username = '${username}'`
            }
        }
        const result = await SQLRequest<any>(sql)
        const transformedResult = transformPlayerState(result)
        let sql2
        for(const player of transformedResult){
            if(sql2 === undefined){
                sql2 = `SELECT * FROM CharacterStatsState WHERE entity_id = '${player.entity_id}'`
            }else{
                sql2 + ` or entity_id = '${player.entity_id}'`
            }
        }
        const result2 = await SQLRequest<any>(sql2)
        console.log(result2)
}
RequestAllPlayerInfo()