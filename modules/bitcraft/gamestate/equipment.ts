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
function transformPlayerState(input: any[][]){
    let PlayerStateArray: Array<PlayerState> = []
    for(const row of input[0]){
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

type ItemRefrence = {
    item_id: Number,
    quantity: Number
}

type EquipmentSlots = {
    main_hand?: ItemRefrence
    off_hand?: ItemRefrence
    head_artifact?: ItemRefrence
    torso_artifact?: ItemRefrence
    hand_artifact?: ItemRefrence
    feet_artifact?: ItemRefrence
}
type EquipmentStateRow = {
    entity_id: Number,
    equipment_slots: EquipmentSlots
}
export function getEquipmentRowsFromRows(rows: any[][]) {
    const EquipmentRows: EquipmentStateRow[] = []
    for (const row of rows) {
        EquipmentRows.push(getEquipmentRowFromRow(row))
    }
    return EquipmentRows
}

function getEquipmentRowFromRow(row: any[]){
    const PlayerState: EquipmentStateRow = {
        entity_id: row[0] as unknown as number,
        equipment_slots: {}
    }
    //@ts-ignore
    if(Object.values(row[1][0][0])[0].length > 2){
        PlayerState.equipment_slots.main_hand = {
            //@ts-ignore
            item_id:  Object.values(row[1][0][0])[0][0],
            //@ts-ignore
            quantity:  Object.values(row[1][0][0])[0][1]
         }
    }
     //@ts-ignore
    if(Object.values(row[1][1][0])[0].length > 2){
        PlayerState.equipment_slots.off_hand = {
            //@ts-ignore
            item_id:  Object.values(row[1][1][0])[0][0],
            //@ts-ignore
            quantity:  Object.values(row[1][1][0])[0][1]
         }
    }
     //@ts-ignore
    if(Object.values(row[1][2][0])[0].length > 2){
        PlayerState.equipment_slots.head_artifact = {
            //@ts-ignore
            item_id:  Object.values(row[1][2][0])[0][0],
            //@ts-ignore
            quantity:  Object.values(row[1][2][0])[0][1]
         }
    }
     //@ts-ignore
    if(Object.values(row[1][3][0])[0].length > 2){
        PlayerState.equipment_slots.torso_artifact = {
            //@ts-ignore
            item_id:  Object.values(row[1][3][0])[0][0],
            //@ts-ignore
            quantity:  Object.values(row[1][3][0])[0][1]
         }
    }
     //@ts-ignore
    if(Object.values(row[1][4][0])[0].length > 2){
        PlayerState.equipment_slots.hand_artifact = {
            //@ts-ignore
            item_id:  Object.values(row[1][4][0])[0][0],
            //@ts-ignore
            quantity:  Object.values(row[1][4][0])[0][1]
         }
    }
     //@ts-ignore
    if(Object.values(row[1][5][0])[0].length > 2){
        PlayerState.equipment_slots.feet_artifact = {
            //@ts-ignore
            item_id:  Object.values(row[1][5][0])[0][0],
            //@ts-ignore
            quantity:  Object.values(row[1][5][0])[0][1]
         }
    }
    return PlayerState
}
export async function SqlRequestEquipmentByEntityId(entitys: {
    entity_id: number
}[]) {
    let sql= ""
    for(const player of entitys){
        if(sql.length === 0){
            sql = `SELECT * FROM EquipmentState WHERE entity_id = ${player.entity_id}`
        }else{
            sql + ` or entity_id = '${player.entity_id}'`
        }
    }
    const result = await SQLRequest<any>(sql)
    return result.row
}