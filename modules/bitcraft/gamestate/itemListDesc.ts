import { readFileSync } from "node:fs";
import { getSome } from "./entity";
import { getItemRefrenceFromRow, getItemsRefrenceFromRows, type ItemRefrence } from "./item";

export type ItemListRow = {
  unique_id: number;
  id: number;
  name: string;
  probability: number;
  items: ItemRefrence[];
  item_list_id: number;
};
export function getItemListRowsFromRows(rows: any[][]) {
  const itemRows: ItemListRow[] = [];
  for (const row of rows) {
    itemRows.push(getItemListRowFromRow(row));
  }

  return itemRows;
}
function getItemListRowFromRow(i: any[]) {
  return {
    unique_id: i[0],
    id: i[1],
    name: i[2],
    probability: i[3],
    items: getItemsRefrenceFromRows(i[4]),
    item_list_id: i[5],
  };
}

export function readItemListRows() {
  return JSON.parse(
    readFileSync(`${process.cwd()}/storage/Desc/ItemListDesc.json`, "utf8"),
  )[0].rows;
}
