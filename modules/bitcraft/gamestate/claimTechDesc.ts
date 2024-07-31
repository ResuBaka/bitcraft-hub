import { readFileSync } from "node:fs";

export interface ClaimTechDescRow {
  id: number;
  description: string;
  tier: number;
  supplies_cost: number;
  research_time: number;
  requirements: number[];
  input: Array<Record<string, any> | Record<string, number> | number>;
  members: number;
  area: number;
  supply: number;
}

let ClaimTechDescState: ClaimTechDescRow[] = [];

export function saveParsedInventorys(rows: ClaimTechDescRow[]): void {
  ClaimTechDescState = rows;
}

export function reloadClaimTechDesc() {
  const inventoryRows = readClaimTechDescRows();
  const parsedInventoryRows = parseClaimTechDescs(inventoryRows);
  saveParsedInventorys(parsedInventoryRows);
}

export function parseClaimTechDescs(rows: any[]): ClaimTechDescRow[] {
  const localInventoryStateRows: ClaimTechDescRow[] = [];

  for (const row of rows) {
    localInventoryStateRows.push(getClaimTechDescRowFromRow(row));
  }

  return localInventoryStateRows;
}

export function getClaimTechDescs(): ClaimTechDescRow[] {
  if (ClaimTechDescState.length === 0) {
    reloadClaimTechDesc();
  }

  return ClaimTechDescState;
}

export function getClaimTechDescRowFromRow(row: any[]): ClaimTechDescRow {
  return {
    id: row[0],
    description: row[1],
    tier: row[2],
    supplies_cost: row[3],
    research_time: row[4],
    requirements: row[5],
    input: row[6],
    members: row[7],
    area: row[8],
    supply: row[9],
  };
}

export function readClaimTechDescRows(): any[] {
  try {
    return JSON.parse(
      readFileSync(`${process.cwd()}/storage/Desc/ClaimTechDesc.json`, "utf8"),
    )[0].rows;
  } catch {
    console.log("No claim tech desc");
    return [];
  }
}
