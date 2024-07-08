export default async function SQLRequest<T>(body: string): Promise<T> {
  const response = await fetch(
    " https://playtest.spacetimedb.org/database/sql/bitcraft-alpha-2",
    {
      method: "POST",
      headers: {
        Authorization:
          "Basic dG9rZW46ZXlKMGVYQWlPaUpLVjFRaUxDSmhiR2NpT2lKRlV6STFOaUo5LmV5Sm9aWGhmYVdSbGJuUnBkSGtpT2lJeFlUZ3pNbVl4WkRRM09XWTJaams0T0dKbE1UaGlZemd4TXpJd1pXVmpNMk15WkdNeVkyWXhObVJoWXpJME9HTXpPR1l5WXpnMFl6WTRNR1pqTldZNUlpd2lhV0YwSWpveE56SXdORFV5TlRrd0xDSmxlSEFpT201MWJHeDkuWjJuREFlQXJjQmpYMG9JZmw0QlNxeE5sMFY4SWdMWF80OTV4RVBOZmZNbjhMREJMNXg1TWFPd1pBSHBIRllJLTM0aUw2WFp5YUlsazdsVGctZ29IMHc=",
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
  const response = await fetch(
    "https://alpha-playtest-1.spacetimedb.com/database/sql/bitcraft-alpha",
    {
      method: "POST",
      headers: {
        Authorization:
          "Basic " + btoa("token:eyJ0eXAiOiJKV1QiLCJhbGciOiJFUzI1NiJ9.eyJoZXhfaWRlbnRpdHkiOiJjZjAxMTQzZGIxMzllYzQ4NTA0YzVhMDlkN2M2ZDYyNmY0YzRiMGM0OGZjMzQ1NzY5ZDhkZWI5ZDliNGNjOTkwIiwiaWF0IjoxNzIwNDUxMjAyLCJleHAiOm51bGx9.D_kgqgSoJemRyA15Ag9RGRFkIv1ohfhXpostUvHEuRzlMu1UoUT2tBC0Imt4l6j7YtuAScr8cS2ljtdf9ljMxA"),
      },
      body: body,
    },
  );

  if (!response.ok) {
    throw new Error(await response.text());
  }

  return response.body;
}
