import WebSocket from "ws";
import { getBuildingDescIdMapFromRows, readBuildingDescRows } from "~/modules/bitcraft/gamestate/buildingDesc";
import { SqlRequesttBuildingStateByClaimEntityId, getBuildingStateRowsFromRows } from "~/modules/bitcraft/gamestate/buildingState";
import { SqlRequestClaimDescriptionByPlayerEntityId, getClaimDescriptionRowsFromRows } from "~/modules/bitcraft/gamestate/claimDescription";
import { SQLQueryInventoryByEntityId, diffItemsInInventorys, getInventoryRowFromRow } from "~/modules/bitcraft/gamestate/inventory";
import { ExpendedRefrence, ItemRefrence } from "~/modules/bitcraft/gamestate/item";
import { PlayerStateRow, SqlRequestAllPlayers, SqlRequestPlayersByUsername, getPlayerEntityIdMapFromRows, getPlayerRowsFromRows } from "~/modules/bitcraft/gamestate/player";
import { SqlRequestAllUsers, getUserMapFromRows } from "~/modules/bitcraft/gamestate/userState";
import {readFile,writeFile} from "node:fs/promises";
import path from "node:path";

const storagePath = `${process.cwd()}/storage/Inventory`
let counter = 0
export default defineNitroPlugin(async (nitroApp) => {
    const usersByIdenity = getUserMapFromRows(await SqlRequestAllUsers())
    const PlayerByEntityId = getPlayerEntityIdMapFromRows(await SqlRequestAllPlayers())
    let websocket: WebSocket | null = null
    try {
        websocket = new WebSocket("wss://alpha-playtest-1.spacetimedb.com/database/subscribe/bitcraft-alpha", "v1.text.spacetimedb", {
            headers: {
                "Authorization": "Basic dG9rZW46ZXlKMGVYQWlPaUpLVjFRaUxDSmhiR2NpT2lKRlV6STFOaUo5LmV5Sm9aWGhmYVdSbGJuUnBkSGtpT2lJeFpXUXlZelJsWVRsbVlUVmtaVFZqTURKaVltVTBNMkV6TldFd05XSTVabVZsTlRVek9ESmhPR0l5WldZd04yRTVaVEk0TnprMU1qUXlPR1ZqTVdFNUlpd2lhV0YwSWpveE56RXpOVFkwTkRZekxDSmxlSEFpT201MWJHeDkua2cyUHBfQ0t5OE1hcTJBT0xDeW0tckRneENkaS01MUZZV05JZ0VhQjJhMnB0YVNTRk11cGdUOXFOVWp3NVlfYkxHOERGcV8yRkxTLWhBRmVmbEU2SFE=",
                "Sec-WebSocket-Protocol": "v1.text.spacetimedb",
                "Sec-WebSocket-Key": "dGhlIHNhbXBsZSBub25jZQ==",
            },
            protocolVersion: 13,
        })

        websocket.on("error", (error) => {
            console.error("Error with bitcraft websocket connection :: ", error)
        })
        websocket.on("open", async () => {
            console.log("Connected")
            let usernames = [
                "Ryuko"
            ]
            const players = getPlayerRowsFromRows(await SqlRequestPlayersByUsername(usernames))
            const claim = getClaimDescriptionRowsFromRows( await SqlRequestClaimDescriptionByPlayerEntityId(players))
            const buildingState = getBuildingStateRowsFromRows(await SqlRequesttBuildingStateByClaimEntityId(claim))
            const buildingDescMap = getBuildingDescIdMapFromRows(readBuildingDescRows())
            const chests = buildingState.filter((buildingState) =>{
                const buildingDesc = buildingDescMap.get(buildingState.building_description_id)
                if(buildingDesc === undefined){
                    return false
                }
                if(buildingDesc.name.includes("Chest")){
                    return true
            
                }
                if( buildingDesc.name.includes("Stockpile")){
                    return true
                }
                return false
            })
            websocket.send(JSON.stringify({
                "subscribe": {
                    query_strings: [
                        SQLQueryInventoryByEntityId(chests)
                    ]
                }
            }))
        })
        websocket.on("message", async (data: any) => {
            const jsonData = JSON.parse(data.toString())
            //console.log(JSON.stringify(jsonData, null, 2))
            if(jsonData.TransactionUpdate !== undefined){
                const callerIdentiy: string = jsonData.TransactionUpdate.event.caller_identity
                const table_updates = jsonData.TransactionUpdate.subscription_update.table_updates[0].table_row_operations
                const oldInventory = getInventoryRowFromRow(table_updates[0].row)
                const info: {
                    identity: string, 
                    playerName?: string
                    playerEntityId?: number
                    timestamp: number
                    diff: {
                    [key: number]: {
                        old: ExpendedRefrence | undefined;
                        new: ExpendedRefrence | undefined;
                    };
                }} = {timestamp: jsonData.TransactionUpdate.event.timestamp, identity: callerIdentiy, diff: diffItemsInInventorys(oldInventory,getInventoryRowFromRow(table_updates[1].row))}
                const user = usersByIdenity.get(callerIdentiy)
                if(user !== undefined){
                    info.playerEntityId = user
                    info.playerName = PlayerByEntityId.get(user)?.username
                }
                let data: any[]
                try{
                    const file = await readFile(`${storagePath}/${oldInventory.entity_id}.json`)
                    data = JSON.parse(file.toString())
                }catch{
                    data = []
                }
                data.push(info)
                await writeFile(`${storagePath}/${oldInventory.entity_id}.json`,JSON.stringify(data,null,3))
            }
        })
        websocket.on("close", (a) => {
            console.log("Disconnected")
            console.error(a)
            console.log("Disconnected")
        })

        Object.assign(nitroApp, { websocket })
    } catch (error) {
        console.error("Error with bitcraft websocket connection :: ", error)
    }
});