import SQLRequest from "../runtime/SQLRequest";
import type { Entity } from "./entity";
import { readFileSync } from "node:fs";
import {
  parseInventorys,
  readInventoryRows,
  saveParsedInventorys,
} from "~/modules/bitcraft/gamestate/inventory";

export interface BuildingStateRow extends Entity {
  claim_entity_id: number;
  direction_index: number;
  building_description_id: number;
  constructed_by_player_entity_id: number;
  nickname: string;
}

let BuildingStateState: BuildingStateRow[] = [];

export function getBuildingStateRowsFromRows(): BuildingStateRow[] {
  if (BuildingStateState.length === 0) {
    BuildingStateState = readBuildingStateRows().map((row) =>
      getBuildingStateRowFromRow(row),
    );
  }

  return BuildingStateState;
}

export function reloadBuildingState() {
  const buildingRows = readBuildingStateRows();
  const parsedBuildingRows = buildingRows.map((row) =>
    getBuildingStateRowFromRow(row),
  );
  saveParsedBuildingState(parsedBuildingRows);
}

function saveParsedBuildingState(rows: BuildingStateRow[]): void {
  BuildingStateState = rows;
}

function getBuildingStateRowFromRow(row: any[]): BuildingStateRow {
  return {
    entity_id: row[0],
    claim_entity_id: row[1],
    direction_index: row[2],
    building_description_id: row[3],
    constructed_by_player_entity_id: row[4],
    nickname: row[5],
  };
}

export async function SqlRequestBuildingStateByConstuctorPlayerEntityId(
  entitys: Entity[],
): Promise<any> {
  let sql = "";

  for (const entity of entitys) {
    if (sql.length === 0) {
      sql = `SELECT * FROM BuildingState WHERE constructed_by_player_entity_id = ${entity.entity_id}`;
    } else {
      sql + ` or constructed_by_player_entity_id = '${entity.entity_id}'`;
    }
  }

  const result = await SQLRequest<any>(sql);

  return result[0].rows;
}

export async function SqlRequesttBuildingStateByClaimEntityId(
  entitys: Entity[],
): Promise<any> {
  let sql = "";

  for (const entity of entitys) {
    if (sql.length === 0) {
      sql = `SELECT * FROM BuildingState WHERE claim_entity_id = ${entity.entity_id}`;
    } else {
      sql = sql + ` or claim_entity_id = ${entity.entity_id}`;
    }
  }

  const result = await SQLRequest<any>(sql);

  return result[0].rows;
}

export function readBuildingStateRows(): any[] {
  return JSON.parse(
    readFileSync(`${process.cwd()}/storage/State/BuildingState.json`, "utf8"),
  )[0].rows;
}
