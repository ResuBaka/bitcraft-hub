import type { Entity } from "./entity";
import { readFileSync } from "node:fs";
import {
  getItemFromItemId,
  getItemRowsFromRows,
  getItemsRefrenceFromRows,
  readItemRows,
  type ExpendedRefrence,
  type ItemRefrence,
  type ItemRow,
} from "./item";
import {
  getCagoDescFromCargoId,
  getCargoDescRowsFromRows,
  readCargoDescRows,
  type CargoDescRow,
} from "./cargoDesc";

export interface TradingOrderStateRow extends Entity {
  building_entity_id: number;
  remaining_stock: any;
  offer_items: ItemRefrence[] | ExpendedRefrence[];
  offer_cargo_id: number[];
  offer_cargo?: CargoDescRow[];
  required_items: ItemRefrence[];
  required_cargo_id: number[];
  required_cargo?: CargoDescRow[];
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

export function replaceTradeOrdersCargoIdWithCargo(
  rows: any,
  cargo_rows: CargoDescRow[],
) {
  const list = [];
  for (const row of rows) {
    list.push(replaceTradeOrderCargoIdWithCargo(row, cargo_rows));
  }
  return list;
}

export function replaceItems(
  itemRows: ItemRow[],
  itemRefrences: ItemRefrence[],
) {
  const list = [];
  for (const itemRefrence of itemRefrences) {
    list.push(getItemFromItemId(itemRows, itemRefrence));
  }
  return list;
}

export function replaceCargos(
  itemRows: CargoDescRow[],
  itemRefrences: number[],
) {
  const list = [];
  if (!itemRefrences) return list;

  for (const itemRefrence of itemRefrences) {
    list.push(getCagoDescFromCargoId(itemRows, itemRefrence));
  }
  return list;
}

export function replaceTradeOrderItemIdWithItem(
  tradeOrder: TradingOrderStateRow,
  items: ItemRow[],
) {
  const expendedTradeOrder: TradingOrderStateRow = {
    ...tradeOrder,
    offer_items: replaceItems(items, tradeOrder.offer_items),
    required_items: replaceItems(items, tradeOrder.required_items),
  };
  return expendedTradeOrder;
}

export function replaceTradeOrdersItemIdWithItem(rows: any, items: ItemRow[]) {
  const list = [];
  for (const row of rows) {
    list.push(replaceTradeOrderItemIdWithItem(row, items));
  }
  return list;
}
export function replaceTradeOrderCargoIdWithCargo(
  tradeOrder: TradingOrderStateRow,
  cargo_rows: CargoDescRow[],
) {
  const expendedTradeOrder: TradingOrderStateRow = {
    ...tradeOrder,
    offer_cargo: replaceCargos(cargo_rows, tradeOrder.offer_cargo_id),
    required_cargo: replaceCargos(cargo_rows, tradeOrder.required_cargo_id),
  };
  return expendedTradeOrder;
}

export function readTradeOrderStateRows() {
  return JSON.parse(
    readFileSync(`${process.cwd()}/storage/State/TradeOrderState.json`, "utf8"),
  )[0].rows;
}
