import SQLRequest from "../runtime/SQLRequest";
import type { Entity } from "./entity";
import { readFileSync } from "node:fs";
import { getItemsRefrenceFromRows, type ItemRefrence } from "./item";

interface TradingOrderStateRow extends Entity {
  building_entity_id: number;
  remaining_stock: any;
  offer_items: ItemRefrence[];
  offer_cargo_id: any;
  required_items: any;
  required_cargo_id: any;
}

export function getTradingOrderStateRowsFromRows(rows: any) {
  const BuildingStateRow: TradingOrderStateRow[] = [];
  for (const row of rows) {
    BuildingStateRow.push(getTradingOrderStateRowFromRow(row));
  }
  return BuildingStateRow;
}
function getTradingOrderStateRowFromRow(row: any[]) {
  const BuildingStateRow: TradingOrderStateRow = {
    entity_id: row[0],
    building_entity_id: row[1],
    remaining_stock: row[2],
    offer_items: getItemsRefrenceFromRows(row[3]),
    offer_cargo_id: row[4],
    required_items: getItemsRefrenceFromRows(row[5]),
    required_cargo_id: row[6],
  };
  return BuildingStateRow;
}

export function readTradeOrderStateRows() {
  return JSON.parse(
    readFileSync(`${process.cwd()}/storage/State/TradeOrderState.json`, "utf8"),
  )[0].rows;
}
