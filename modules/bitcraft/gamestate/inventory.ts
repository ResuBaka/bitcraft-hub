import SQLRequest from "../runtime/SQLRequest";
import {getItemFromItemId, getItemRefrenceFromRow, getItemRowsFromRows, getItemsRefrenceFromRow, readItemRows, type ItemRefrence } from "../gamestate/item"
import type { Entity } from "./entity";
type ItemSlot = {
    volume: number
    contents?: ItemRefrence
}
interface  InventoryStateRow extends Entity {
    pockets: ItemSlot[]
    inventory_index: number
    cargo_index: number
    owner_entity_id: number
    
}

function getItemSlots(rows: any){
    const itemRows: ItemSlot[] = []
    for (const row of rows) {
        itemRows.push(getItemSlot(row))
    }
    return itemRows
}
function getItemSlot(row: any){
    const InventoryState: ItemSlot = {
        volume: row[0],
        contents: getItemRefrenceFromRow(row[1]),
    }
    return InventoryState
}

export function getInventoryRowsFromRows(rows: any){
    const PlayerStateRow: InventoryStateRow[] = []
    for (const row of rows) {
        PlayerStateRow.push(getInventoryRowFromRow(row))
    }
    return PlayerStateRow
}
function getInventoryRowFromRow(row: any[]){
    const InventoryState: InventoryStateRow = {
        entity_id: row[0],
        pockets: getItemSlots(row[1]),
        inventory_index: row[2],
        cargo_index: row[3],
        owner_entity_id: row[4]
    }
    return InventoryState
}

export function replaceInventoryItemsIdWithItems(rows: any){
    for (const row of rows) {
        replaceInventoryItemIdWithItem(row)
    }
    return rows
}
export function replaceInventoryItemIdWithItem(inventory: InventoryStateRow){
    const items = getItemRowsFromRows(readItemRows())
    for(const pocket  of inventory.pockets){
        if(pocket.contents !== undefined){
            const item = getItemFromItemId(items,pocket.contents)
            //@ts-ignore
            pocket.contents = item
        }
    }
    return inventory
}
export async function SqlRequestInventoryByEntityId(entitys: Entity[]) {
    let sql= ""
    for(const entity of entitys){
        if(sql.length === 0){
            sql = `SELECT * FROM InventoryState WHERE owner_entity_id = ${entity.entity_id}`
        }else{
            sql + ` or owner_entity_id = '${entity.entity_id}'`
        }
    }
    const result = await SQLRequest<any>(sql)
    return result[0].rows
}