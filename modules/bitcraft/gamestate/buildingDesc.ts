import SQLRequest from "../runtime/SQLRequest";
import { readFileSync } from "node:fs";
interface BuildingDescRow {
  id: number;
  functions: any;
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

export function getBuildingDescRowsFromRows(rows: any) {
  const BuildingStateRow: BuildingDescRow[] = [];
  for (const row of rows) {
    BuildingStateRow.push(getBuildingDescRowFromRow(row));
  }
  return BuildingStateRow;
}
function getBuildingDescRowFromRow(row: any[]) {
  const BuildingStateRow: BuildingDescRow = {
    id: row[0],
    functions: row[1],
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
  return BuildingStateRow;
}
export function getBuildingDescIdMapFromRows(rows: any) {
  const BuildingStateRow: Map<number, BuildingDescRow> = new Map();
  for (const row of rows) {
    const data = getBuildingDescRowFromRow(row);
    BuildingStateRow.set(data.id, data);
  }
  return BuildingStateRow;
}
export async function SqlRequestBuildingDesc() {
  const result = await SQLRequest<any>(`SELECT * BuildingDesc`);
  return result[0].rows;
}

export function readBuildingDescRows() {
  return JSON.parse(
    readFileSync(`${process.cwd()}/storage/Desc/BuildingDesc.json`, "utf8"),
  )[0].rows;
}
