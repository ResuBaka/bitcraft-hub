import WebSocket from "ws";

export default defineNitroPlugin(async (nitroApp) => {
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