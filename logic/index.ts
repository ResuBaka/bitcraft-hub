import { readFile, writeFile } from "node:fs/promises";

let items: Item[] = [];
let buildings: Building[] = [];

let buildingsFile = `${process.cwd()}/storage/buildings.json`;
let itemsFile = `${process.cwd()}/storage/items.json`;

export async function loadItems() {
  try {
    await readFile(itemsFile);
  } catch (e) {
    console.error("Items file not found, creating new one", e);
    await writeFile(itemsFile, "[]");
  }

  items = JSON.parse(await readFile(itemsFile, "utf-8"));
}

export async function loadBuildings() {
  try {
    await readFile(buildingsFile);
  } catch (e) {
    console.error("Buildings file not found, creating new one", e);
    await writeFile(buildingsFile, "[]");
  }

  buildings = JSON.parse(await readFile(buildingsFile, "utf-8"));
}

export async function addItem(item: any) {
  const exists = items.find((i) => i.id === item.id);
  if (exists) {
    throw new Error(`Item with id ${item.id} already exists`);
  }

  items.push(item);
  await saveItems();
}

export async function updateItem(item: any) {
  const index = items.findIndex((i) => i.id === item.id);

  if (index === -1) {
    throw new Error(`Item with id ${item.id} does not exists`);
  }

  items[index] = item;
  await saveItems();
}

export async function deleteItem(itemId: string) {
  const index = items.findIndex((item) => item.id === itemId);

  if (index === -1) {
    throw new Error(`Item with id ${itemId} does not exists`);
  }

  items.splice(index, 1);

  await saveItems();
}

export async function addBuilding(item: Building) {
  const exists = buildings.find((i) => i.id === item.id);
  if (exists) {
    throw new Error(`Building with id ${item.id} already exists`);
  }

  buildings.push(item);
  await saveItems();
}

export async function updateBuilding(item: Building) {
  const index = buildings.findIndex((i) => i.id === item.id);

  if (index === -1) {
    throw new Error(`Building with id ${item.id} does not exists`);
  }

  buildings[index] = item;
  await saveBuilding();
}

export async function deleteBuilding(itemId: string) {
  const index = buildings.findIndex((item) => item.id === itemId);

  if (index === -1) {
    throw new Error(`Building with id ${itemId} does not exists`);
  }

  buildings.splice(index, 1);

  await saveBuilding();
}

export function getItem(id: string) {
  return items.find((item) => item.id === id);
}
export function getBuilding(id: string) {
  return buildings.find((item) => item.id === id);
}

export function getAllItem(itemId: string): FullItem | undefined {
  const topItem = items.find((item) => item.id === itemId);

  if (!topItem) {
    return undefined;
  }

  const neededItems =
    topItem?.items?.map((neededItemId) => {
      let item = items.find((item) => item.id === neededItemId.id);

      if (item && item.items.length > 0) {
        item = getAllItem(item.id);
      }

      if (!item) {
        return null;
      }

      return {
        ...item,
        amount: neededItemId.amount,
      };
    }) || [];

  return {
    ...topItem,
    items: neededItems.filter((item) => item !== null),
  };
}

export async function saveItems() {
  await writeFile(itemsFile, JSON.stringify(items, null, 2));
}
export async function saveBuilding() {
  await writeFile(buildingsFile, JSON.stringify(items, null, 2));
}

export { items, buildings };
