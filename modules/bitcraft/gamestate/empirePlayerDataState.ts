import SQLRequest from "../runtime/SQLRequest";
import type { Entity } from "./entity";
import { readFileSync } from "node:fs";

export interface EmpirePlayerDataStateRow extends Entity {
  empire_entity_id: number;
  rank: number;
  donated_shards: number;
  noble: number;

}

export function getEmpirePlayerDataStateRowsFromRows(
  rows: any[],
): EmpirePlayerDataStateRow[] {
  const PlayerStateRow: EmpirePlayerDataStateRow[] = [];

  for (const row of rows) {
    PlayerStateRow.push(getEmpirePlayerDataStateRowFromRow(row));
  }

  return PlayerStateRow;
}

function getEmpirePlayerDataStateRowFromRow(row: any[]): EmpirePlayerDataStateRow {
  return {
    entity_id: row[0],
    empire_entity_id: row[1],
    rank:  row[2],
    donated_shards: row[3],
    noble:  row[4],
  };
}

export function readEmpirePlayerDataState(): any[] {
  return JSON.parse(
    readFileSync(
      `${process.cwd()}/storage/State/EmpirePlayerDataState.json`,
      "utf8",
    ),
  )[0].rows;
}
