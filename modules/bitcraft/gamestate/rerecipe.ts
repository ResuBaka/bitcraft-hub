import { readFileSync } from "node:fs";
import { getItemFromItemId, type ExpendedRefrence, type ItemRefrence, type ItemRow } from "./item";
import type { ItemListRow } from "./itemListDesc";

interface ItemStack extends ItemRefrence {
  discovery_score: 1;
  consumption_chance: 1;
  item?: ExpendedRefrence
};
interface ItemStackWithInner extends ItemStack {
  inner?: CraftingRecipeRow[];
};
type CraftingRecipeRow = {
  id: number;
  name: string;
  time_requirement: number;
  stamina_requirement: number;
  building_requirement: Record<string, any>;
  level_requirements: Array<any>;
  tool_requirements: Array<any>;
  consumed_item_stacks: ItemStack[] | ItemStackWithInner[];
  discovery_triggers: Array<number>;
  required_knowledges: Array<number>;
  required_claim_tech_id: Array<number>;
  full_discovery_score: number;
  completion_experience: Array<number>;
  allow_use_hands: boolean;
  crafted_item_stacks: ItemStack[] | ItemStackWithInner[];
  is_passive: boolean;
  actions_required: number;
  tool_mesh_index: number;
  animation_start: string;
  animation_end: string;
};

export function getCraftingRecipesFromRows(rows: any[][]) {
  const craftingRecipes: CraftingRecipeRow[] = [];
  for (const row of rows) {
    craftingRecipes.push(getCraftingRecipeFromRow(row));
  }

  return craftingRecipes;
}

function getCraftingRecipeFromRow(i: any[]) {
  return {
    id: i[0],
    name: i[1],
    time_requirement: i[2],
    stamina_requirement: i[3],
    building_requirement: tobuildingRequirement(i[4]),
    level_requirements: toLevelRequirement(i[5]),
    tool_requirements: toToolRequirement(i[6]),
    consumed_item_stacks: toConsumedItemStacksRequirement(i[7]),
    discovery_triggers: i[8],
    required_knowledges: i[9],
    required_claim_tech_id: i[10],
    full_discovery_score: i[11],
    completion_experience: toCompletionExperienceRequirement(i[12]),
    allow_use_hands: i[13],
    crafted_item_stacks: toCraftedItemStacksRequirement(i[14]),
    is_passive: i[15],
    actions_required: i[16],
    tool_mesh_index: i[17],
    animation_start: i[18],
    animation_end: i[19],
  };
}

function tobuildingRequirement(rows: Record<string, any>) {
  const temp = [];
  for (const key of Object.keys(rows)) {
    const value = rows[key];

    if (value.length === 2) {
      temp.push({
        type: value[0],
        tier: value[1],
      });
    } else {
      temp.push(key);
    }
  }

  return temp;
}

function toLevelRequirement(rows: number[][]) {
  const temp = [];
  for (const row of rows) {
    temp.push({
      skill_id: row[0],
      level: row[1],
    });
  }

  return temp;
}
function toToolRequirement(rows: number[][]) {
  const temp = [];
  for (const row of rows) {
    temp.push({
      tool_type: row[0],
      level: row[1],
      power: row[2],
    });
  }

  return temp;
}

function toConsumedItemStacksRequirement(rows: number[][]) {
  const temp = [];
  for (const row of rows) {
    temp.push({
      item_id: row[0],
      quantity: row[1],
      item_type: row[2],
      discovery_score: row[3],
      consumption_chance: row[4],
    });
  }

  return temp;
}

function toCompletionExperienceRequirement(rows: number[][]) {
  const temp = [];
  for (const row of rows) {
    temp.push({
      skill_id: row[0],
      quantity: row[1],
    });
  }

  return temp;
}

function toCraftedItemStacksRequirement(rows: number[][]) {
  const temp = [];
  for (const row of rows) {
    temp.push({
      item_id: row[0],
      quantity: row[1],
      item_type: row[2],
      durability: row[3],
    });
  }

  return temp;
}
export function getAllConsumedItemsFromItem(
  rows: CraftingRecipeRow[],
  item_id: number,
  items: ItemRow[],
  items_list: ItemListRow[]
): CraftingRecipeRow[] {

  const posibilities_item_list = items_list.filter((item_list) => 
    item_list.items.filter((item) => {
      return item.item_id == item_id;
    }).length > 0,
  );
  const posibilities_item_list_array = posibilities_item_list.map((item_list) => item_list.id)
  const posibilities_items = items.filter((item) => 
    posibilities_item_list_array.includes(item.item_list_id)
  );
  const posibilities_items_array = posibilities_items.map((item_list) => item_list.id)
  const posibilities = rows.filter(
    (recipe) =>
      recipe.crafted_item_stacks.filter((cis) => {
        return cis.item_id == item_id || posibilities_items_array.includes(cis.item_id);
      }).length > 0,
  );

  const list = [];
  const cache = {}
  for (const posibilitie of posibilities) {
    list.push(
      getAllConsumedItemsFromStack(rows, posibilitie,items,items_list, [posibilitie.id], cache),
    );
  }
  return list;
}

export function getAllConsumedItemsFromStack(
  rows: CraftingRecipeRow[],
  item: CraftingRecipeRow,
  items: ItemRow[],
  items_list: ItemListRow[],
  alreadyUsed: number[],
  cache: {[key: string] : any}
): CraftingRecipeRow {
  for (const itemstack of item.consumed_item_stacks as ItemStackWithInner[]) {
    const posibilities_item_list = items_list.filter((item_list) => {
      return item_list.items.filter((item) => {
        return item.item_id == itemstack.item_id;
      }).length > 0
    });
    const posibilities_item_list_array = posibilities_item_list.map((item_list) => item_list.id)
    const posibilities_items = items.filter((item) => 
      posibilities_item_list_array.includes(item.item_list_id)
    );
    const posibilities_items_array = posibilities_items.map((item_list) => item_list.id)

    const posibilities = rows.filter(
      (recipe) =>
        recipe.crafted_item_stacks.filter((cis) => {
          return cis.item_id == itemstack.item_id  || posibilities_items_array.includes(cis.item_id);
        }).length > 0,
    );


    const list = [];
    for (const posibilitie of posibilities) {
      if (alreadyUsed.includes(posibilitie.id)) {
        continue;
      }
      if(cache[posibilitie.id] !== undefined){
        list.push(cache[posibilitie.id])
        continue
      }
      list.push(
        getAllConsumedItemsFromStack(rows, posibilitie,items,items_list, [
          ...alreadyUsed,
          posibilitie.id,
        ], cache),
      );
    }
    itemstack.inner = list
  }
  const object  = {}
  const consumed_item_stacks = []
  for(const itemstack of item.consumed_item_stacks){
    const itemFromId = getItemFromItemId(items,itemstack)
    let newItem = {
      item_id: itemFromId.item_id,
      name: itemFromId?.item?.name,
      quantity: itemFromId.quantity,
      inner: itemstack.inner
    }
    consumed_item_stacks.push(newItem)
  }
  object.consumed_item_stacks = consumed_item_stacks
  const crafted_item_stacks = []
  for(const itemstack of item.crafted_item_stacks){
    const itemFromId = getItemFromItemId(items,itemstack)
    let newItem = {
      item_id: itemFromId.item_id,
      name: itemFromId?.item?.name,
      quantity: itemFromId.quantity,
      inner: itemstack.inner
    }
    crafted_item_stacks.push(newItem)
  }
  object.crafted_item_stacks = crafted_item_stacks
  object.name = item.name
  cache[item.id] = object
  return object;
}
export function readCraftingRecipeRows(): any[][] {
  return JSON.parse(
    readFileSync(
      `${process.cwd()}/storage/Desc/CraftingRecipeDesc.json`,
      "utf8",
    ),
  )[0].rows;
}
