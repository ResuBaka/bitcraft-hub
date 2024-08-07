import SQLRequest from "../runtime/SQLRequest";
import {
  type ExpendedRefrence,
  getItemFromItemId,
  getItemRefrenceFromRow,
  getItemRowsFromRows,
  type ItemRefrence,
  type ItemRow,
  readItemRows,
} from "../gamestate/item";
import { type Entity, getSome } from "./entity";
import { readFileSync } from "node:fs";

export type ItemSlot = {
  volume: number;
  contents?: ItemRefrence;
};
const items = getItemRowsFromRows();
export interface InventoryStateRow extends Entity {
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

function getItemSlots(rows: any): ItemSlot[] {
  const itemRows: ItemSlot[] = [];
  for (const row of rows) {
    itemRows.push(getItemSlot(row));
  }
  return itemRows;
}

function getItemSlot(row: any): ItemSlot {
  const contents = getSome(row[1]);

  return {
    volume: row[0],
    contents:
      contents !== undefined ? getItemRefrenceFromRow(contents) : undefined,
  };
}

let InventoryStateRows: InventoryStateRow[] = [];

export function saveParsedInventorys(rows: InventoryStateRow[]): void {
  InventoryStateRows = rows;
}

export function reloadInventoryState() {
  const inventoryRows = readInventoryRows();
  const parsedInventoryRows = parseInventorys(inventoryRows);
  saveParsedInventorys(parsedInventoryRows);
}

export function parseInventorys(rows: any[]): InventoryStateRow[] {
  const localInventoryStateRows: InventoryStateRow[] = [];

  for (const row of rows) {
    localInventoryStateRows.push(getInventoryRowFromRow(row));
  }

  return localInventoryStateRows;
}

export function getInventorys(): InventoryStateRow[] {
  if (InventoryStateRows.length === 0) {
    reloadInventoryState();
  }

  return InventoryStateRows;
}

export function getInventoryRowFromRow(row: any[]): InventoryStateRow {
  return {
    entity_id: row[0],
    pockets: getItemSlots(row[1]),
    inventory_index: row[2],
    cargo_index: row[3],
    owner_entity_id: row[4],
  };
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

  const oldInv = replaceInventoryItemIdWithItem(oldInventory, items);
  const newInv = replaceInventoryItemIdWithItem(newInventory, items);

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
export function replaceInventoryItemsIdWithItems(
  rows: InventoryStateRow[],
  items: ItemRow[],
): InventoryStateRow[] {
  for (const row of rows) {
    replaceInventoryItemIdWithItem(row, items);
  }

  return rows;
}

export function replaceInventoryItemIdWithItem(
  inventory: InventoryStateRow,
  items: ItemRow[],
): InventoryStateRow {
  for (const pocket of inventory.pockets) {
    if (pocket.contents !== undefined) {
      pocket.contents = getItemFromItemId(items, pocket.contents);
    }
  }

  return inventory;
}

export function SQLQueryInventoryByEntityId(entitys: Entity[]): string {
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

export async function SqlRequestInventoryByEntityId(
  entitys: Entity[],
): Promise<any> {
  const result = await SQLRequest<any>(SQLQueryInventoryByEntityId(entitys));
  return result[0].rows;
}

export function readInventoryRows() {
  try {
    return JSON.parse(
      readFileSync(
        `${process.cwd()}/storage/State/InventoryState.json`,
        "utf8",
      ),
    )[0].rows;
  } catch {
    console.log("No inventory state");
    return [];
  }
}

export function readInventroyChanges(id: number): false | InventoryChanged[] {
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

  lineLoop: for (const line of lines) {
    if (line.length === 0) {
      continue;
    }

    const data = JSON.parse(line);

    for (const diffEntry in data.diff) {
      const diff = data.diff[diffEntry];
      if (diff.old !== undefined && diff.new !== undefined) {
        if (itemWasMoved(diff)) {
          continue lineLoop;
        }
      }
    }

    list.push(data);
  }

  return list;
}

function itemWasMoved(diff: any): boolean {
  return (
    diff.old.item_id === diff.new.item_id &&
    diff.old.quantity === diff.new.quantity
  );
}
