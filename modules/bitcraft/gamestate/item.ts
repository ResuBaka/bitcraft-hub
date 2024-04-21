import {readFileSync} from "node:fs";

export type ItemRow = {
    id:    number
    name:    string
    description:    string
    volume:    number
    durability:    number
    secondary_knowledge_id:    number
    model_asset_name:    string
    icon_asset_name:    string
    tier:    number
    tag:    string
    rarity:   Record<string, any>
    compendium_entry:    boolean
    item_list_id:    number
}
export type ItemRefrence = {
    item_id: Number,
    quantity: Number
}


export function getItemRowsFromRows(rows: any[][]) {
    const itemRows: ItemRow[] = []
    for (const row of rows) {
        itemRows.push(getItemRowFromRow(row))
    }


    return itemRows
}
export function getItemFromItemId(items: ItemRow[], item_refrence: ItemRefrence) {
    return items.filter((item) => item.id === item_refrence.item_id)[0]
}
function getItemRowFromRow(i: any[]) {
    return {
        id: i[0],
        name: i[1],
        description: i[2],
        volume: i[3],
        durability: i[4],
        secondary_knowledge_id: i[5],
        model_asset_name: i[6],
        icon_asset_name: i[7],
        tier: i[8],
        tag: i[9],
        rarity: i[10],
        compendium_entry: i[11],
        item_list_id: i[12]
    }
}

export function readItemRows() {
    return JSON.parse(readFileSync(`${process.cwd()}/storage/Desc/ItemDesc.json`, 'utf8'))[0].rows;
}