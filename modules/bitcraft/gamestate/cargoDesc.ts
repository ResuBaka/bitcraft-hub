import { readFileSync } from "node:fs";

export interface CargoDescRow {
  id: number;
  name: string;
  description: string;
  volume: number;
  secondary_knowledge_id: number;
  model_asset_name: string;
  icon_asset_name: string;
  carried_model_asset_name: string;
  pick_up_animation_start: string;
  pick_up_animation_end: string;
  drop_animation_start: string;
  drop_animation_end: string;
  pick_up_time: number;
  place_time: number;
  animator_state: string;
  movement_modifier: number;
  blocks_path: boolean;
  on_destroy_yield_cargos: number[];
  despawn_time: number;
  tier: number;
  tag: string;
  rarity: any;
}

export function getCargoDescRowsFromRows(rows: any): CargoDescRow[] {
  const BuildingStateRow: CargoDescRow[] = [];

  for (const row of rows) {
    BuildingStateRow.push(getCargoDescRowFromRow(row));
  }

  return BuildingStateRow;
}

export function getCagoDescFromCargoId(
  cargo_rows: CargoDescRow[],
  cargo_id: number,
): CargoDescRow {
  return cargo_rows.filter((cargo) => cargo.id === cargo_id)[0];
}

function getCargoDescRowFromRow(row: any[]): CargoDescRow {
  return {
    id: row[0],
    name: row[1],
    description: row[2],
    volume: row[3],
    secondary_knowledge_id: row[4],
    model_asset_name: row[5],
    icon_asset_name: row[6],
    carried_model_asset_name: row[7],
    pick_up_animation_start: row[8],
    pick_up_animation_end: row[9],
    drop_animation_start: row[10],
    drop_animation_end: row[11],
    pick_up_time: row[12],
    place_time: row[13],
    animator_state: row[14],
    movement_modifier: row[15],
    blocks_path: row[16],
    on_destroy_yield_cargos: row[17],
    despawn_time: row[18],
    tier: row[19],
    tag: row[20],
    rarity: row[21],
  };
}

export function readCargoDescRows(): any[] {
  return JSON.parse(
    readFileSync(`${process.cwd()}/storage/Desc/CargoDesc.json`, "utf8"),
  )[0].rows;
}
