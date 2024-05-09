import type { Entity } from "./entity";
import { readFileSync } from "node:fs";

export interface TradingOrderStateRow extends Entity {
  spawn_timestamp: number;
  description_id: number;
  direction: number;
}

export function getCargoStateRowsFromRows(rows: any[]): TradingOrderStateRow[] {
  const BuildingStateRow: TradingOrderStateRow[] = [];
  for (const row of rows) {
    BuildingStateRow.push(getCargoStateRowFromRow(row));
  }
  return BuildingStateRow;
}
function getCargoStateRowFromRow(row: any[]): TradingOrderStateRow {
  const BuildingStateRow: TradingOrderStateRow = {
    entity_id: row[0],
    spawn_timestamp: row[1],
    description_id: row[2],
    direction: row[3],
  };
  return BuildingStateRow;
}

export function readCargoStateRows(): any[] {
  return JSON.parse(
    readFileSync(`${process.cwd()}/storage/State/CargoState.json`, "utf8"),
  )[0].rows;
}
