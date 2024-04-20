import {getItemRowsFromRows, readItemRows} from "~/modules/bitcraft/gamestate/item";

export default defineEventHandler((event) => {
    let {
        tag,
        tier,
        search,
        page,
    } = getQuery(event)

    const rows = getItemRowsFromRows(readItemRows())

    const perPage = 30
    if (tier) {
        tier = parseInt(tier)
    }

    if (page) {
        page = parseInt(page)
    } else {
        page = 1
    }

    const rowsFilterted = rows?.filter((item: any) => {
        return (!tag || item.tag === tag) &&
            (!tier || item.tier === tier) &&
            (!search || item.name.toLowerCase().includes(search.toLowerCase()))
    }) ?? []

    return {
        items: rowsFilterted.slice((page - 1) * perPage, page * perPage),
        total: rowsFilterted.length,
        tags: Array.from(new Set(rows.map((item: any) => item.tag))),
        tiers: Array.from(new Set(rows.map((item: any) => parseInt(item.tier)))),
        page,
        perPage,
    }
})
