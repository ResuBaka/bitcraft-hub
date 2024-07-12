import SQLRequest from "../runtime/SQLRequest";
import type { Entity } from "./entity";
import { readFileSync } from "node:fs";

export interface EmpireStateRow extends Entity {
  capital_building_entity_id: number;
  name: string;
  color1_index: number;
  color2_index: number;
  shard_treasury: number;
  directive_message: string;
  directive_message_timestamp: number;
  nobility_threshold: number;
  num_claims: number;
}

export function getEmpireStateRowsFromRows(
  rows: any[],
): EmpireStateRow[] {
  const PlayerStateRow: EmpireStateRow[] = [];

  for (const row of rows) {
    PlayerStateRow.push(getEmpireStateRowFromRow(row));
  }

  return PlayerStateRow;
}

function getEmpireStateRowFromRow(row: any[]): EmpireStateRow {
  return {
    entity_id: row[0],
    capital_building_entity_id: row[1],
    name: row[2],
    color1_index: row[3],
    color2_index: row[4],
    shard_treasury: row[5],
    directive_message: row[6],
    directive_message_timestamp: row[7],
    nobility_threshold: row[8],
    num_claims: row[9],
  };
}

export function readEmpireState(): any[] {
  return JSON.parse(
    readFileSync(
      `${process.cwd()}/storage/State/EmpireState.json`,
      "utf8",
    ),
  )[0].rows;
}
