import {
  getAllConsumedItemsFromItem,
  getCraftingRecipesFromRows,
  readCraftingRecipeRows,
} from "~/modules/bitcraft/gamestate/rerecipe";

const rows = getCraftingRecipesFromRows(readCraftingRecipeRows());

export default defineEventHandler((event) => {
  const { neededInCrafting, producedInCrafting, neededToCraft } = getQuery(event);

  if (neededInCrafting) {
    return rows.filter(
      (recipe) =>
        recipe.consumed_item_stacks.filter((cis) => {
          return cis.item_id == neededInCrafting;
        }).length > 0,
    );
  }
  if (neededToCraft) {
    console.log(neededToCraft)
    return getAllConsumedItemsFromItem(rows,neededToCraft)
    return
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
