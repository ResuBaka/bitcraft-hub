import type { Entity } from "./entity";
import { readFileSync } from "node:fs";

export interface VehicleStateRow extends Entity {
  owner_id: Number;
  direction: Number;
  vehicle_description_id: Number;
  nickname: string;
}

let VehicleStateState: VehicleStateRow[] = [];

export function getVehicleState(): VehicleStateRow[] {
  if (VehicleStateState.length === 0) {
    const playerRows: VehicleStateRow[] = [];
    const rows = readVehicleStateRows();

    for (const row of rows) {
      playerRows.push(parseVehicleStateRow(row));
    }

    VehicleStateState = playerRows;
  }

  return VehicleStateState;
}

export function reloadVehicleState() {
  const rows = readVehicleStateRows();
  const parsedPlayerRows = rows.map((row) => parseVehicleStateRow(row));
  saveParsedVehicleState(parsedPlayerRows);
}

function saveParsedVehicleState(rows: VehicleStateRow[]): void {
  VehicleStateState = rows;
}

function parseVehicleStateRow(row: any[]): VehicleStateRow {
  return {
    entity_id: row[0] as unknown as number,
    owner_id: row[1] as unknown as number,
    direction: row[2],
    vehicle_description_id: row[3] as unknown as number,
    nickname: row[4] as unknown as string,
  };
}

export function readVehicleStateRows(): any[] {
  return JSON.parse(
    readFileSync(`${process.cwd()}/storage/State/VehicleState.json`, "utf8"),
  )[0].rows;
}
