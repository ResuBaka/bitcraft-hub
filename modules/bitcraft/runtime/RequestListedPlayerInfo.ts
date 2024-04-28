import {getPlayerRowsFromRows, SqlRequestPlayersByUsername } from "../gamestate/player"
import { getClaimDescriptionRowsFromRows, SqlRequestClaimDescriptionByPlayerEntityId } from "../gamestate/claimDescription";
import { getBuildingStateRowsFromRows, SqlRequesttBuildingStateByClaimEntityId } from "../gamestate/buildingState";
import { getBuildingDescIdMapFromRows, readBuildingDescRows } from "../gamestate/buildingDesc";
import { SQLQueryInventoryByEntityId, SqlRequestInventoryByEntityId } from "../gamestate/inventory";
import { Identity } from "@clockworklabs/spacetimedb-sdk";
import SQLRequest from "./SQLRequest";
let usernames = [
    "Ryuko"
]

export default async function RequestAllPlayerInfo() {
    console.log(await SqlRequestPlayersByUsername(usernames))
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
    const data = await SqlRequestInventoryByEntityId(chests)
    const identity = new Identity("682c346aa967aff098ab0b202dfb1e14960c22762f147f0b22a8b5e40c1e08a9")
    console.log(JSON.stringify(await SQLRequest(`SELECT * FROM UserState where UserState.identity='682c346aa967aff098ab0b202dfb1e14960c22762f147f0b22a8b5e40c1e08a9'`),null,3))
}
RequestAllPlayerInfo()