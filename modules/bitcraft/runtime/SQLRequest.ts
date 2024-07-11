export default async function SQLRequest<T>(body: string): Promise<T> {
  const auth = useRuntimeConfig().bitcraft.auth;
  const response = await fetch(
    " https://playtest.spacetimedb.org/database/sql/bitcraft-alpha-2",
    {
      method: "POST",
      headers: {
        Authorization: `Basic ${btoa(`${auth.username}:${auth.password}`)}`,
      },
      body: body,
    },
  );

  if (!response.ok) {
    throw new Error(await response.text());
  }

  return await response.json();
}

export async function SQLRequestStream(body: string) {
  const auth = useRuntimeConfig().bitcraft.auth;
  const response = await fetch(
    "https://playtest.spacetimedb.org/database/sql/bitcraft-alpha-2",
    {
      method: "POST",
      headers: {
        Authorization: `Basic ${btoa(`${auth.username}:${auth.password}`)}`,
      },
      body: body,
    },
  );

  if (!response.ok) {
    throw new Error(await response.text());
  }

  return response.body;
}
