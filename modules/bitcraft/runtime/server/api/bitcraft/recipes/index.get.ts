import {
  getAllConsumedItemsFromItem,
  getCraftingRecipesFromRows,
  readCraftingRecipeRows,
} from "~/modules/bitcraft/gamestate/rerecipe";
import type { TradingOrderStateRow } from "~/modules/bitcraft/gamestate/tradeOrder";

const rows = getCraftingRecipesFromRows(readCraftingRecipeRows());

export type TradeOrderQuery = {
  neededInCrafting?: number;
  producedInCrafting?: number;
  neededToCraft?: number;
};

export default defineEventHandler((event) => {
  const { neededInCrafting, producedInCrafting, neededToCraft } =
    getQuery<TradeOrderQuery>(event);

  if (neededInCrafting) {
    return rows.filter(
      (recipe) =>
        recipe.consumed_item_stacks.filter((cis) => {
          return cis.item_id == neededInCrafting;
        }).length > 0,
    );
  }
  if (neededToCraft) {
    return getAllConsumedItemsFromItem(rows, neededToCraft);
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
