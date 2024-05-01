import SQLRequest from "../runtime/SQLRequest";
import type { Entity } from "./entity";

interface userStateRow extends Entity {
  identity: string;
}
export function getUserowsFromRows(rows: any[][]) {
  const playerRows: userStateRow[] = [];
  for (const row of rows) {
    playerRows.push(getUserRowFromRow(row));
  }
  return playerRows;
}
export function getUserMapFromRows(rows: any[][]) {
  const playerRows: Map<string, number> = new Map();
  for (const row of rows) {
    const user = getUserRowFromRow(row);
    playerRows.set(user.identity, user.entity_id);
  }
  return playerRows;
}
function getUserRowFromRow(row: any[]) {
  const PlayerState: userStateRow = {
    entity_id: row[0] as unknown as number,
    identity: row[1][0],
  };
  return PlayerState;
}

export async function SqlRequestAllUsers() {
  const result = await SQLRequest<any>(`SELECT * FROM UserState`);
  return result[0].rows;
}
