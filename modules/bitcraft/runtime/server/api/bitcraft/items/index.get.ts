import {getItemRowsFromRows, readItemRows} from "~/modules/bitcraft/gamestate/item";

export default defineEventHandler(() => {
    const rows = readItemRows()

    return getItemRowsFromRows(rows)
})
