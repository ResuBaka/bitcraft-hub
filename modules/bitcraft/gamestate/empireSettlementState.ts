import SQLRequest from "../runtime/SQLRequest";
import type { Entity } from "./entity";
import { readFileSync } from "node:fs";

export interface EmpireSettlementStateRow extends Entity {
  empire_entity_id: number;
  chunk_index: number;
  can_house_empire_storehouse: number;
  members_donations: number;
  location: any,


}

export function getEmpireSettlementStateRowsFromRows(
  rows: any[],
): EmpireSettlementStateRow[] {
  const PlayerStateRow: EmpireSettlementStateRow[] = [];

  for (const row of rows) {
    PlayerStateRow.push(getEmpireSettlementStateRowFromRow(row));
  }

  return PlayerStateRow;
}

function getEmpireSettlementStateRowFromRow(row: any[]): EmpireSettlementStateRow {
  return {
    entity_id: row[0],
    empire_entity_id: row[1],
    chunk_index: row[2],
    can_house_empire_storehouse: row[3],
    members_donations: row[4],
    location: row[5],
  };
}

export function readEmpireSettlementState(): any[] {
  return JSON.parse(
    readFileSync(
      `${process.cwd()}/storage/State/EmpireSettlementState.json`,
      "utf8",
    ),
  )[0].rows;
}
