import {getClaimDescriptionRowsFromRows, readClaimRows} from "~/modules/bitcraft/gamestate/claimDescription";

const rows = getClaimDescriptionRowsFromRows(readClaimRows())
export default defineEventHandler((event) => {
    const id = getRouterParam(event, "id", { decode: true });

    if (!id) {
        throw createError({
            statusCode: 400,
            statusMessage: "Missing ID",
        });
    }

    const claims = rows.find((claims) => claims.entity_id == parseInt(id))

    if (!claims) {
        throw createError({
            statusCode: 404,
            statusMessage: "Claim was not found",
        });
    }

    return claims
})
