import { readFile, writeFile } from "node:fs/promises";
import type {
  Profession,
  Building,
  Item,
  Npc,
  Root,
  Requirement,
} from "~/types";

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

export function getBuildingIndex(buildingId: string) {
  return data.buildings.findIndex(({ id }) => id === buildingId);
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

  data.buildings.splice(index, 1);

  await saveRoot();
}

export function getBuildingRequirement(
  buildingId: string,
  requirementUUID: string,
) {
  const building = getBuilding(buildingId);

  if (!building) {
    throw new Error("Building not found");
  }

  console.log("Building", building, requirementUUID);
  const index = building.requirement.findIndex(
    ({ uuid }) => uuid === requirementUUID,
  );

  if (index == -1) {
    throw new Error("Requirement not found");
  }

  return building.requirement[index];
}

export async function updateBuildingRequirement(
  buildingId: string,
  requirement: Requirement,
) {
  const building = getBuilding(buildingId);

  if (!building) {
    throw new Error("Building not found");
  }

  const index = building.requirement.findIndex(
    ({ uuid }) => uuid === requirement.uuid,
  );

  if (index == -1) {
    throw new Error("Requirement not found");
  }

  const buildingIndex = getBuildingIndex(buildingId);

  data.buildings[buildingIndex].requirement[index] = requirement;

  await saveRoot();
}

export async function addBuildingRequirement(
  buildingId: string,
  requirement: Requirement,
) {
  const building = getBuilding(buildingId);

  if (!building) {
    throw new Error("Building not found");
  }

  const index = building.requirement.findIndex(
    ({ uuid }) => uuid === requirement.uuid,
  );

  if (index > -1) {
    throw new Error("Requirement with Id already exists");
  }

  const buildingIndex = getBuildingIndex(buildingId);

  data.buildings[buildingIndex].requirement.push(requirement);

  await saveRoot();
}

export async function deleteBuildingRequirement(
  buildingId: string,
  requirementUUID: string,
) {
  const building = getBuilding(buildingId);

  if (!building) {
    throw new Error("Building not found");
  }

  const index = building.requirement.findIndex(
    ({ uuid }) => uuid === requirementUUID,
  );

  if (index == -1) {
    throw new Error("Requirement not found");
  }
  const buildingIndex = getBuildingIndex(buildingId);

  data.buildings[buildingIndex].requirement.splice(index, 1);

  await saveRoot();
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

export function getProfession(professionId: string) {
  return data.professions.find(({ id }) => id === professionId);
}

export async function addProfession(profession: Profession) {
  const index = data.professions.findIndex(({ id }) => id === profession.id);

  if (index > -1) {
    throw new Error("Profession already exists");
  }

  data.professions.push(profession);

  await saveRoot();

  return data.professions[index];
}

export async function updateProfession(profession: Profession) {
  const index = data.professions.findIndex(({ id }) => id === profession.id);

  if (index == -1) {
    return null;
  }

  data.professions[index] = profession;

  await saveRoot();

  return data.professions[index];
}

export async function deleteProfession(professionId: string) {
  const index = data.professions.findIndex(({ id }) => id === professionId);

  if (index == -1) {
    throw new Error("No Profession exists");
  }

  await saveRoot();

  data.professions.splice(index, 1);
}
