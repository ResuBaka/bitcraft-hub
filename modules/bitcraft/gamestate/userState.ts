import SQLRequest from "../runtime/SQLRequest";
import type { Entity } from "./entity";

export interface UserStateRow extends Entity {
  identity: string;
}

export function getUserowsFromRows(rows: any[]): UserStateRow[] {
  const playerRows: UserStateRow[] = [];

  for (const row of rows) {
    playerRows.push(getUserRowFromRow(row));
  }

  return playerRows;
}

export function getUserMapFromRows(rows: any[]): Map<string, number> {
  const playerRows: Map<string, number> = new Map();

  for (const row of rows) {
    const user = getUserRowFromRow(row);
    playerRows.set(user.identity, user.entity_id);
  }

  return playerRows;
}

function getUserRowFromRow(row: any[]): UserStateRow {
  return {
    entity_id: row[0] as unknown as number,
    identity: row[1][0],
  };
}

export async function SqlRequestAllUsers(): Promise<any> {
  const result = await SQLRequest<any>(`SELECT * FROM UserState`);
  return result[0].rows;
}
