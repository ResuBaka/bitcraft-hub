import SQLRequest from "../runtime/SQLRequest";
import { readFileSync } from "node:fs";

export interface BuildingDescRow {
  id: number;
  functions: BuildingDescFunction[];
  name: string;
  description: string;
  rested_buff_duration: number;
  light_radius: number;
  model_asset_name: string;
  icon_asset_name: string;
  unenterable: boolean;
  wilderness: boolean;
  footprint: any;
}

export interface BuildingDescFunction {
  function_type: number;
  level: number;
  crafting_slots: number;
  storage_slots: number;
  cargo_slots: number;
  refining_slots: number;
  refining_cargo_slots: number;
  cargo_slot_size: number;
  trade_orders: number;
  buff_ids: number[];
}

let BuildingDescState: BuildingDescRow[] = [];

export function getBuildingDescRowsFromRows(): BuildingDescRow[] {
  if (BuildingDescState.length === 0) {
    const BuildingStateRow: BuildingDescRow[] = [];
    const rows = readBuildingDescRows();

    for (const row of rows) {
      BuildingStateRow.push(getBuildingDescRowFromRow(row));
    }

    BuildingDescState = BuildingStateRow;
  }

  return BuildingDescState;
}

export function reloadBuildingDesc() {
  const rows = readBuildingDescRows();
  const parsedBuildingDescRows = rows.map((row) =>
    getBuildingDescRowFromRow(row),
  );
  saveParsedBuildingDesc(parsedBuildingDescRows);
}

function saveParsedBuildingDesc(rows: BuildingDescRow[]): void {
  BuildingDescState = rows;
}

function getBuildingDescRowFromRow(row: any[]): BuildingDescRow {
  return {
    id: row[0],
    functions: parseFunctions(row[1]),
    name: row[2],
    description: row[3],
    rested_buff_duration: row[4],
    light_radius: row[5],
    model_asset_name: row[6],
    icon_asset_name: row[7],
    unenterable: row[8],
    wilderness: row[9],
    footprint: row[10],
  };
}

function parseFunctions(functions: any[]): any {
  const functionsArray: BuildingDescFunction[] = [];

  for (const functionsArrayElement of functions) {
    functionsArray.push({
      function_type: functionsArrayElement[0],
      level: functionsArrayElement[1],
      crafting_slots: functionsArrayElement[2],
      storage_slots: functionsArrayElement[3],
      cargo_slots: functionsArrayElement[4],
      refining_slots: functionsArrayElement[5],
      refining_cargo_slots: functionsArrayElement[6],
      cargo_slot_size: functionsArrayElement[7],
      trade_orders: functionsArrayElement[8],
      buff_ids: functionsArrayElement[9],
    });
  }

  return functionsArray;
}
const BuildingStateRows: Map<number, BuildingDescRow> = new Map();

export function getBuildingDescIdMapFromRows(): Map<number, BuildingDescRow> {
  if (BuildingStateRows.size === 0) {
    const rows = readBuildingDescRows();
    for (const row of rows) {
      const data = getBuildingDescRowFromRow(row);
      BuildingStateRows.set(data.id, data);
    }
  }

  return BuildingStateRows;
}

export async function SqlRequestBuildingDesc(): Promise<any> {
  const result = await SQLRequest<any>(`SELECT * BuildingDesc`);
  return result[0].rows;
}

export function readBuildingDescRows(): any[] {
  return JSON.parse(
    readFileSync(`${process.cwd()}/storage/Desc/BuildingDesc.json`, "utf8"),
  )[0].rows;
}
