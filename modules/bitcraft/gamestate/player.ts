import SQLRequest from "../runtime/SQLRequest";
import type { Entity } from "./entity";
import { readFileSync } from "node:fs";

export interface PlayerStateRow extends Entity {
  serial_id: Number;
  username: string;
  eth_pub_key: string;
  time_played: Number;
  session_start_timestamp: Number;
  time_signed_in: Number;
  sign_in_timestamp: Number;
  signed_in: boolean;
  unmanned_vehicle_coords: any;
  destination_marker: any;
  favorite_crafting_recipes: any;
  teleport_location: any;
}

export function getPlayerRowsFromRows(rows: any[]): PlayerStateRow[] {
  const playerRows: PlayerStateRow[] = [];
  for (const row of rows) {
    playerRows.push(getPlayerRowFromRow(row));
  }
  return playerRows;
}

export function getPlayerEntityIdMapFromRows(
  rows: any[],
): Map<number, PlayerStateRow> {
  const playerRows: Map<number, PlayerStateRow> = new Map();

  for (const row of rows) {
    const player = getPlayerRowFromRow(row);
    playerRows.set(player.entity_id, player);
  }

  return playerRows;
}

function getPlayerRowFromRow(row: any[]): PlayerStateRow {
  return {
    entity_id: row[0] as unknown as number,
    serial_id: row[1] as unknown as number,
    username: row[2],
    eth_pub_key: row[3],
    time_played: row[4] as unknown as number,
    session_start_timestamp: row[5] as unknown as number,
    time_signed_in: row[6] as unknown as number,
    sign_in_timestamp: row[7] as unknown as number,
    signed_in: row[8] as unknown as boolean,
    unmanned_vehicle_coords: row[9],
    destination_marker: row[10],
    favorite_crafting_recipes: row[11],
    teleport_location: row[12],
  };
}

export async function SqlRequestPlayersByUsername(
  usernames: string[],
): Promise<any> {
  let sql = "";
  for (const username of usernames) {
    if (sql.length === 0) {
      sql = `SELECT * FROM PlayerState WHERE username = '${username}'`;
    } else {
      sql + ` or username = '${username}'`;
    }
  }

  const result = await SQLRequest<any>(sql);

  return result[0].rows;
}

export async function SqlRequestAllPlayers(): Promise<any> {
  const result = await SQLRequest<any>("SELECT * FROM PlayerState");
  return result[0].rows;
}

export function readPlayerStateRows(): any[] {
  return JSON.parse(
    readFileSync(`${process.cwd()}/storage/State/PlayerState.json`, "utf8"),
  )[0].rows;
}
