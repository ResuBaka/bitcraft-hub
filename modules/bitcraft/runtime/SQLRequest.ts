export default async function SQLRequest<T>(body: string): Promise<T> {
    const response =  await fetch("https://alpha-playtest-1.spacetimedb.com/database/sql/bitcraft-alpha", {
        method:"POST",
        headers: {
            "Authorization": "Basic dG9rZW46ZXlKMGVYQWlPaUpLVjFRaUxDSmhiR2NpT2lKRlV6STFOaUo5LmV5Sm9aWGhmYVdSbGJuUnBkSGtpT2lJeFpXUXlZelJsWVRsbVlUVmtaVFZqTURKaVltVTBNMkV6TldFd05XSTVabVZsTlRVek9ESmhPR0l5WldZd04yRTVaVEk0TnprMU1qUXlPR1ZqTVdFNUlpd2lhV0YwSWpveE56RXpOVFkwTkRZekxDSmxlSEFpT201MWJHeDkua2cyUHBfQ0t5OE1hcTJBT0xDeW0tckRneENkaS01MUZZV05JZ0VhQjJhMnB0YVNTRk11cGdUOXFOVWp3NVlfYkxHOERGcV8yRkxTLWhBRmVmbEU2SFE="
        },
        body: body
    })

    if (!response.ok) {
        throw new Error(await response.text());
    }

    return await response.json();
}