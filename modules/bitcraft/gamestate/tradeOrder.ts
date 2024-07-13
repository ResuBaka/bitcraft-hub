import type { Entity } from "./entity";
import { readFileSync } from "node:fs";
import {
  type ExpendedRefrence,
  getItemFromItemId,
  getItemsRefrenceFromRows,
  type ItemRefrence,
  type ItemRow,
} from "./item";
import { type CargoDescRow, getCagoDescFromCargoId } from "./cargoDesc";

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

let TradingOrderStateState: TradingOrderStateRow[] = [];

export function getTradingOrderStateRowsFromRows(): TradingOrderStateRow[] {
  if (TradingOrderStateState.length === 0) {
    const BuildingStateRow: TradingOrderStateRow[] = [];
    const rows = readTradeOrderStateRows();

    for (const row of rows) {
      BuildingStateRow.push(getTradingOrderStateRowFromRow(row));
    }

    return BuildingStateRow;
  }

  return TradingOrderStateState;
}

export function reloadTradingOrderState() {
  const rows = readTradeOrderStateRows();
  const parsedTradingOrderStateRows = rows.map((row) =>
    getTradingOrderStateRowFromRow(row),
  );
  saveParsedTradingOrderState(parsedTradingOrderStateRows);
}

function saveParsedTradingOrderState(rows: TradingOrderStateRow[]): void {
  TradingOrderStateState = rows;
}

function getTradingOrderStateRowFromRow(row: any[]): TradingOrderStateRow {
  return {
    entity_id: row[0],
    building_entity_id: row[1],
    remaining_stock: row[2],
    offer_items: getItemsRefrenceFromRows(row[3]),
    offer_cargo_id: row[4],
    required_items: getItemsRefrenceFromRows(row[5]),
    required_cargo_id: row[6],
  };
}

export function replaceTradeOrdersCargoIdWithCargo(
  rows: any[],
  cargo_rows: CargoDescRow[],
): TradingOrderStateRow[] {
  const list: TradingOrderStateRow[] = [];

  for (const row of rows) {
    list.push(replaceTradeOrderCargoIdWithCargo(row, cargo_rows));
  }

  return list;
}

export function replaceItems(
  itemRows: ItemRow[],
  itemRefrences: ItemRefrence[],
): ExpendedRefrence[] {
  const list: ExpendedRefrence[] = [];

  for (const itemRefrence of itemRefrences) {
    list.push(getItemFromItemId(itemRows, itemRefrence));
  }

  return list;
}

export function replaceCargos(
  itemRows: CargoDescRow[],
  itemRefrences: number[],
): CargoDescRow[] {
  if (!itemRefrences) return [];

  const list: CargoDescRow[] = [];

  for (const itemRefrence of itemRefrences) {
    list.push(getCagoDescFromCargoId(itemRows, itemRefrence));
  }
  return list;
}

export function replaceTradeOrderItemIdWithItem(
  tradeOrder: TradingOrderStateRow,
  items: ItemRow[],
): TradingOrderStateRow {
  return {
    ...tradeOrder,
    offer_items: replaceItems(items, tradeOrder.offer_items),
    required_items: replaceItems(items, tradeOrder.required_items),
  };
}

export function replaceTradeOrdersItemIdWithItem(
  rows: any[],
  items: ItemRow[],
): TradingOrderStateRow[] {
  const list: TradingOrderStateRow[] = [];

  for (const row of rows) {
    list.push(replaceTradeOrderItemIdWithItem(row, items));
  }

  return list;
}

export function replaceTradeOrderCargoIdWithCargo(
  tradeOrder: TradingOrderStateRow,
  cargo_rows: CargoDescRow[],
): TradingOrderStateRow {
  return {
    ...tradeOrder,
    offer_cargo: replaceCargos(cargo_rows, tradeOrder.offer_cargo_id),
    required_cargo: replaceCargos(cargo_rows, tradeOrder.required_cargo_id),
  };
}

export function readTradeOrderStateRows() {
  return JSON.parse(
    readFileSync(`${process.cwd()}/storage/State/TradeOrderState.json`, "utf8"),
  )[0].rows;
}
