import {getPlayerRowsFromRows, SqlRequestPlayersByUsername } from "../gamestate/player"
import { getClaimDescriptionRowsFromRows, SqlRequestClaimDescriptionByPlayerEntityId } from "../gamestate/claimDescription";
let usernames = [
    "Hiems Whiterock"
]

export default async function RequestAllPlayerInfo() {
    console.log(await SqlRequestPlayersByUsername(usernames))
    const players = getPlayerRowsFromRows(await SqlRequestPlayersByUsername(usernames))
    console.log(players)
    const inventory = await SqlRequestClaimDescriptionByPlayerEntityId(players)
    console.log(JSON.stringify(getClaimDescriptionRowsFromRows(inventory)))
}
RequestAllPlayerInfo()