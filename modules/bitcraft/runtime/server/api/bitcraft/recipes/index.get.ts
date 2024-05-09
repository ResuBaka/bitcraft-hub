import { getItemRowsFromRows, readItemRows } from "~/modules/bitcraft/gamestate/item";
import { getItemListRowsFromRows, readItemListRows } from "~/modules/bitcraft/gamestate/itemListDesc";
import {
  getAllConsumedItemsFromItem,
  getCraftingRecipesFromRows,
  readCraftingRecipeRows,
} from "~/modules/bitcraft/gamestate/rerecipe";

const rows = getCraftingRecipesFromRows(readCraftingRecipeRows());
const items = getItemRowsFromRows(readItemRows());
const item_list = getItemListRowsFromRows(readItemListRows())

export default defineEventHandler((event) => {
  const { neededInCrafting, producedInCrafting, neededToCraft } =
    getQuery(event);

  if (neededInCrafting) {
    return rows.filter(
      (recipe) =>
        recipe.consumed_item_stacks.filter((cis) => {
          return cis.item_id == neededInCrafting;
        }).length > 0,
    );
  }
  if (neededToCraft) {
    const abc = getAllConsumedItemsFromItem(rows, neededToCraft,items,item_list);
    const stringifyed = JSON.stringify(abc)
    return stringifyed
  }

  if (producedInCrafting) {
    return rows.filter(
      (recipe) =>
        recipe.crafted_item_stacks.filter((cis) => {
          return cis.item_id == producedInCrafting;
        }).length > 0,
    );
  }

  return rows;
});
