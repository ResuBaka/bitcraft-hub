import SQLRequest from "../runtime/SQLRequest";
import type { Entity } from "./entity";
import { readFileSync } from "node:fs";

export interface EmpireRankStateRow extends Entity {
  empire_entity_id: number;
  rank: number;
  title: string;
  permissions: Boolean[]
}

export function getEmpireRankStateRowsFromRows(
  rows: any[],
): EmpireRankStateRow[] {
  const PlayerStateRow: EmpireRankStateRow[] = [];

  for (const row of rows) {
    PlayerStateRow.push(getEmpireRankStateRowFromRow(row));
  }

  return PlayerStateRow;
}

function getEmpireRankStateRowFromRow(row: any[]): EmpireRankStateRow {
  return {
    entity_id: row[0],
    empire_entity_id: row[1],
    rank: row[2],
    title: row[3],
    permissions: row[4],
  };
}

export function readEmpireRankState(): any[] {
  return JSON.parse(
    readFileSync(
      `${process.cwd()}/storage/State/EmpireRankState.json`,
      "utf8",
    ),
  )[0].rows;
}
