import { readFileSync } from "node:fs";

export interface SkillDescRow {
  id: number;
  name: string;
  description: string;
  icon_asset_name: string;
  title: string;
}
export function getSkillRowsFromRows(rows: any[]): SkillDescRow[] {
  const playerRows: SkillDescRow[] = [];
  for (const row of rows) {
    playerRows.push(getSkillRowFromRow(row));
  }
  return playerRows;
}

function getSkillRowFromRow(row: any[]): SkillDescRow {
  const skill: SkillDescRow = {
    id: row[0],
    name: row[1],
    description: row[1],
    icon_asset_name: row[1],
    title: row[1],
  };
  return skill;
}

export function readSkillRows() {
  return JSON.parse(
    readFileSync(`${process.cwd()}/storage/Desc/SkillDesc.json`, "utf8"),
  )[0].rows;
}
