import { readFile, writeFile } from "node:fs/promises";
import type { Building, Item, Npc, Root } from "~/types";

let rootFile = `${process.cwd()}/storage/root.json`;
export let data: Root = {
  professions: [],
  npcs: [],
  buildings: [],
  items: [],
};

function formatData(data: Root) {
  if (import.meta.dev) {
    return JSON.stringify(data, null, 2);
  }

  return JSON.stringify(data);
}

export async function loadRoot() {
  try {
    await readFile(rootFile);
  } catch (e) {
    console.error("Items file not found, creating new one", e);

    await writeFile(rootFile, JSON.stringify(data, null, 2));
  }

  data = JSON.parse(await readFile(rootFile, "utf-8"));
}

export async function saveRoot() {
  await writeFile(rootFile, formatData(data));
}

export function getItem(itemId: string) {
  return data.items.find(({ id }) => id === itemId);
}

export async function addItem(item: Item) {
  const index = data.items.findIndex(({ id }) => id === item.id);

  if (index > -1) {
    throw new Error("Item already exists");
  }

  data.items.push(item);

  await saveRoot();

  return data.items[index];
}

export async function updateItem(item: Item) {
  const index = data.items.findIndex(({ id }) => id === item.id);

  if (index == -1) {
    return null;
  }

  data.items[index] = item;

  await saveRoot();

  return data.items[index];
}

export async function deleteItem(itemId: string) {
  const index = data.items.findIndex(({ id }) => id === itemId);

  if (index == -1) {
    throw new Error("No Item exists");
  }

  await saveRoot();

  data.items.splice(index, 1);
}

export function getBuilding(buildingId: string) {
  return data.buildings.find(({ id }) => id === buildingId);
}

export async function addBuilding(building: Building) {
  const index = data.buildings.findIndex(({ id }) => id === building.id);

  if (index > -1) {
    throw new Error("Building already exists");
  }

  data.buildings.push(building);

  await saveRoot();

  return data.buildings[index];
}

export async function updateBuilding(building: Building) {
  const index = data.buildings.findIndex(({ id }) => id === building.id);

  if (index == -1) {
    return null;
  }

  data.buildings[index] = building;

  await saveRoot();

  return data.buildings[index];
}

export async function deleteBuilding(buildingId: string) {
  const index = data.buildings.findIndex(({ id }) => id === buildingId);

  if (index == -1) {
    throw new Error("No Building exists");
  }

  await saveRoot();

  data.buildings.splice(index, 1);
}

export function getNpc(npcId: string) {
  return data.npcs.find(({ id }) => id === npcId);
}

export async function addNpc(npc: Npc) {
  const index = data.npcs.findIndex(({ id }) => id === npc.id);

  if (index > -1) {
    throw new Error("Npc already exists");
  }

  data.npcs.push(npc);

  await saveRoot();

  return data.npcs[index];
}

export async function updateNpc(npc: Npc) {
  const index = data.npcs.findIndex(({ id }) => id === npc.id);

  if (index == -1) {
    return null;
  }

  data.npcs[index] = npc;

  await saveRoot();

  return data.npcs[index];
}

export async function deleteNpc(npcId: string) {
  const index = data.npcs.findIndex(({ id }) => id === npcId);

  if (index == -1) {
    throw new Error("No Npc exists");
  }

  await saveRoot();

  data.npcs.splice(index, 1);
}
