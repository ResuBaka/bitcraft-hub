import {
  getAllConsumedItemsFromItem,
  getCraftingRecipesFromRows,
} from "~/modules/bitcraft/gamestate/rerecipe";

export type TradeOrderQuery = {
  neededInCrafting?: number;
  producedInCrafting?: number;
  neededToCraft?: number;
};

export default defineEventHandler((event) => {
  const id = getRouterParam(event, "id", { decode: true });
  let idParsed = parseInt(id);

  const rows = getCraftingRecipesFromRows();

  return rows.filter(
    (recipe) =>
      recipe.crafted_item_stacks.filter((cis) => {
        return cis.item_id == idParsed;
      }).length > 0,
  );
});
