import SQLRequest from "../runtime/SQLRequest";
import type { Entity } from "./entity";
interface ClaimMember extends Entity {
    user_name: string
    inventory_permission: boolean
    build_permission: boolean
    officer_permission: boolean
    co_owner_permission: boolean
}
interface  ClaimDescriptionRow extends Entity {
    owner_player_entity_id: number
    owner_building_entity_id: number
    name: string
    supplies: number
    building_maintenance: number
    members: any
    tiles: number
    extensions: number
    neutral: boolean
    location: any
    treasury: number
}

function getClaimMembers(rows: any){
    const itemRows: ClaimMember[] = []
    for (const row of rows) {
        itemRows.push(getClaimMember(row))
    }
    return itemRows
}
function getClaimMember(row: any){
    const InventoryState: ClaimMember = {
        entity_id: row[0],
        user_name: row[1],
        inventory_permission: row[2],
        build_permission: row[3],
        officer_permission: row[4],
        co_owner_permission: row[5]
    }
    return InventoryState
}

export function getClaimDescriptionRowsFromRows(rows: any){
    const PlayerStateRow: ClaimDescriptionRow[] = []
    for (const row of rows) {
        PlayerStateRow.push(getClaimDescriptionRowFromRow(row))
    }
    return PlayerStateRow
}
export function getClaimDescriptionMapFromRows(rows: any){
    const PlayerStateRow: Map<number,ClaimDescriptionRow> = new Map()
    for (const row of rows) {
        const data = getClaimDescriptionRowFromRow(row)
        PlayerStateRow.set(data.entity_id,data)
    }
    return PlayerStateRow
}
function getClaimDescriptionRowFromRow(row: any[]){
    const InventoryState: ClaimDescriptionRow = {
        entity_id: row[0],
        owner_player_entity_id: row[1],
        owner_building_entity_id: row[2],
        name: row[3],
        supplies: row[4],
        building_maintenance: row[5],
        members: getClaimMembers(row[6]),
        tiles: row[7],
        extensions: row[8],
        neutral: row[9],
        location: row[10],
        treasury: row[11],
    }
    return InventoryState
}

export async function SqlRequestClaimDescriptionByPlayerEntityId(entitys: Entity[]) {
    let sql= ""
    for(const entity of entitys){
        if(sql.length === 0){
            sql = `SELECT * FROM ClaimDescriptionState WHERE owner_player_entity_id = ${entity.entity_id}`
        }else{
            sql + ` or owner_player_entity_id = '${entity.entity_id}'`
        }
    }
    const result = await SQLRequest<any>(sql)
    return result[0].rows
}