import { readFileSync } from "node:fs";
import type { Entity } from "~/modules/bitcraft/gamestate/entity";

export interface ClaimTechStateRow extends Entity {
  learned: number[];
  researching: number;
  start_timestamp: number;
  research_time: number;
  cancel_token: any;
}

let ClaimTechStateState: ClaimTechStateRow[] = [];

export function saveParsedInventorys(rows: ClaimTechStateRow[]): void {
  ClaimTechStateState = rows;
}

export function reloadClaimTechState() {
  const inventoryRows = readClaimTechStateRows();
  const parsedInventoryRows = parseClaimTechStates(inventoryRows);
  saveParsedInventorys(parsedInventoryRows);
}

export function parseClaimTechStates(rows: any[]): ClaimTechStateRow[] {
  const localInventoryStateRows: ClaimTechStateRow[] = [];

  for (const row of rows) {
    localInventoryStateRows.push(getClaimTechStateRowFromRow(row));
  }

  return localInventoryStateRows;
}

export function getClaimTechStates(): ClaimTechStateRow[] {
  if (ClaimTechStateState.length === 0) {
    reloadClaimTechState();
  }

  return ClaimTechStateState;
}

export function getClaimTechStateRowFromRow(row: any[]): ClaimTechStateRow {
  return {
    entity_id: row[0],
    learned: row[1],
    researching: row[2],
    start_timestamp: row[3],
    research_time: row[4],
    cancel_token: row[5],
  };
}

export function readClaimTechStateRows(): any[] {
  try {
    return JSON.parse(
      readFileSync(
        `${process.cwd()}/storage/State/ClaimTechState.json`,
        "utf8",
      ),
    )[0].rows;
  } catch {
    console.log("No claim tech state");
    return [];
  }
}
