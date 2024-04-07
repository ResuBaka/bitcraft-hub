import { readFile, writeFile } from "node:fs/promises";
import type { Root } from "~/types";

let rootFile = `${process.cwd()}/storage/root.json`;
let data: Root = {
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
