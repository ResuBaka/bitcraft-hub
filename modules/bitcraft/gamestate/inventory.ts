import SQLRequest from "../runtime/SQLRequest";
import {
  getItemFromItemId,
  getItemRefrenceFromRow,
  getItemRowsFromRows,
  readItemRows,
  type ExpendedRefrence,
  type ItemRefrence,
} from "../gamestate/item";
import { getSome, type Entity } from "./entity";
import { readFileSync } from "node:fs";

type ItemSlot = {
  volume: number;
  contents?: ItemRefrence;
};
interface InventoryStateRow extends Entity {
  pockets: ItemSlot[];
  inventory_index: number;
  cargo_index: number;
  owner_entity_id: number;
}

export type InventoryChanged = {
  inventory_id: number;
  identity: string;
  playerName?: string;
  playerEntityId?: number;
  timestamp: number;
  created?: any;
  deleted?: any;
  diff?: {
    [key: number]: {
      old: ExpendedRefrence | undefined;
      new: ExpendedRefrence | undefined;
    };
  };
};

function getItemSlots(rows: any) {
  const itemRows: ItemSlot[] = [];
  for (const row of rows) {
    itemRows.push(getItemSlot(row));
  }
  return itemRows;
}
function getItemSlot(row: any) {
  const contents = getSome(row[1]);
  const InventoryState: ItemSlot = {
    volume: row[0],
    contents:
      contents !== undefined ? getItemRefrenceFromRow(contents) : undefined,
  };
  return InventoryState;
}

export function getInventoryRowsFromRows(rows: any) {
  const PlayerStateRow: InventoryStateRow[] = [];
  for (const row of rows) {
    PlayerStateRow.push(getInventoryRowFromRow(row));
  }
  return PlayerStateRow;
}
export function getInventoryRowFromRow(row: any[]) {
  const InventoryState: InventoryStateRow = {
    entity_id: row[0],
    pockets: getItemSlots(row[1]),
    inventory_index: row[2],
    cargo_index: row[3],
    owner_entity_id: row[4],
  };
  return InventoryState;
}
export function diffItemsInInventorys(
  oldInventory: InventoryStateRow,
  newInventory: InventoryStateRow,
) {
  let diff: {
    [key: number]: {
      old: ExpendedRefrence | undefined;
      new: ExpendedRefrence | undefined;
    };
  } = {};
  const oldInv = replaceInventoryItemIdWithItem(oldInventory);
  const newInv = replaceInventoryItemIdWithItem(newInventory);
  for (const pocketIndex of oldInventory.pockets.keys()) {
    const oldItem = JSON.stringify(oldInv.pockets[pocketIndex].contents);
    const newItem = JSON.stringify(newInv.pockets[pocketIndex].contents);
    if (oldItem !== newItem) {
      diff[pocketIndex] = {
        old: oldInv.pockets[pocketIndex].contents as
          | ExpendedRefrence
          | undefined,
        new: newInv.pockets[pocketIndex].contents as
          | ExpendedRefrence
          | undefined,
      };
    }
  }
  return diff;
}
export function replaceInventoryItemsIdWithItems(rows: any) {
  for (const row of rows) {
    replaceInventoryItemIdWithItem(row);
  }
  return rows;
}
export function replaceInventoryItemIdWithItem(inventory: InventoryStateRow) {
  const items = getItemRowsFromRows(readItemRows());
  for (const pocket of inventory.pockets) {
    if (pocket.contents !== undefined) {
      const item = getItemFromItemId(items, pocket.contents);
      //@ts-ignore
      pocket.contents = item;
    }
  }
  return inventory;
}
export function SQLQueryInventoryByEntityId(entitys: Entity[]) {
  let sql = "";
  for (const entity of entitys) {
    if (sql.length === 0) {
      sql = `SELECT * FROM InventoryState WHERE owner_entity_id = ${entity.entity_id}`;
    } else {
      sql = sql + ` or owner_entity_id = ${entity.entity_id}`;
    }
  }
  return sql;
}

export async function SqlRequestInventoryByEntityId(entitys: Entity[]) {
  const result = await SQLRequest<any>(SQLQueryInventoryByEntityId(entitys));
  return result[0].rows;
}

export function readInventoryRows() {
  return JSON.parse(
    readFileSync(`${process.cwd()}/storage/State/InventoryState.json`, "utf8"),
  )[0].rows;
}

export function readInventroyChanges(id: number) {
  let file;
  try {
    file = readFileSync(
      `${process.cwd()}/storage/Inventory/${id}.json`,
      "utf8",
    );
  } catch {
    return false;
  }
  const lines = file.split("\n");
  const list: InventoryChanged[] = [];
  for (const line of lines) {
    if (line.length > 0) {
      list.push(JSON.parse(line));
    }
  }
  return list;
}
