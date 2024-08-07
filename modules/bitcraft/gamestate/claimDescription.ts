import SQLRequest from "../runtime/SQLRequest";
import type { Entity } from "./entity";
import { readFileSync } from "node:fs";

export interface ClaimMember extends Entity {
  user_name: string;
  inventory_permission: boolean;
  build_permission: boolean;
  officer_permission: boolean;
  co_owner_permission: boolean;
}

export interface ClaimDescriptionRow extends Entity {
  owner_player_entity_id: number;
  owner_building_entity_id: number;
  name: string;
  supplies: number;
  building_maintenance: number;
  members: any[];
  tiles: number;
  extensions: number;
  neutral: boolean;
  location: any;
  treasury: number;
}

function getClaimMembers(rows: any): ClaimMember[] {
  const itemRows: ClaimMember[] = [];

  for (const row of rows) {
    itemRows.push(getClaimMember(row));
  }

  return itemRows;
}

function getClaimMember(row: any): ClaimMember {
  return {
    entity_id: row[0],
    user_name: row[1],
    inventory_permission: row[2],
    build_permission: row[3],
    officer_permission: row[4],
    co_owner_permission: row[5],
  };
}

let ClaimDescriptionState: ClaimDescriptionRow[] = [];

export function getClaimDescriptionRowsFromRows(): ClaimDescriptionRow[] {
  if (ClaimDescriptionState.length === 0) {
    const PlayerStateRow: ClaimDescriptionRow[] = [];
    const rows = readClaimRows();

    for (const row of rows) {
      PlayerStateRow.push(getClaimDescriptionRowFromRow(row));
    }

    ClaimDescriptionState = PlayerStateRow;
  }

  return ClaimDescriptionState;
}

export function reloadClaimDescription() {
  const rows = readClaimRows();
  const parsedClaimRows = rows.map((row) => getClaimDescriptionRowFromRow(row));
  saveParsedClaimDescription(parsedClaimRows);
}

function saveParsedClaimDescription(rows: ClaimDescriptionRow[]): void {
  ClaimDescriptionState = rows;
}

export function getClaimDescriptionMapFromRows(
  rows: any[],
): Map<number, ClaimDescriptionRow> {
  const PlayerStateRow: Map<number, ClaimDescriptionRow> = new Map();

  for (const row of rows) {
    const data = getClaimDescriptionRowFromRow(row);
    PlayerStateRow.set(data.entity_id, data);
  }

  return PlayerStateRow;
}

function getClaimDescriptionRowFromRow(row: any[]): ClaimDescriptionRow {
  return {
    entity_id: row[0],
    owner_player_entity_id: row[1],
    owner_building_entity_id: row[2],
    name: row[3],
    supplies: row[4],
    building_maintenance: row[5],
    members: getClaimMembers(row[6]),
    tiles: row[7],
    extensions: row[8],
    neutral: row[9],
    location: row[10],
    treasury: row[11],
  };
}

export async function SQLRequestBuildingDescAllClaims(): Promise<any> {
  const result = await SQLRequest<any>(
    "SELECT * FROM BuildingState where claim_entity_id > 0 ",
  );

  return result[0].rows;
}

export async function SqlRequestClaimDescriptionByPlayerEntityId(
  entitys: Entity[],
): Promise<any> {
  let sql = "";

  for (const entity of entitys) {
    if (sql.length === 0) {
      sql = `SELECT * FROM ClaimDescriptionState WHERE owner_player_entity_id = ${entity.entity_id}`;
    } else {
      sql + ` or owner_player_entity_id = '${entity.entity_id}'`;
    }
  }

  const result = await SQLRequest<any>(sql);

  return result[0].rows;
}

export function readClaimRows(): any[] {
  return JSON.parse(
    readFileSync(
      `${process.cwd()}/storage/State/ClaimDescriptionState.json`,
      "utf8",
    ),
  )[0].rows;
}
