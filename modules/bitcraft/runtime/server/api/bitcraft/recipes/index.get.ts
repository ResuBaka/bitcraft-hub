import { getCraftingRecipesFromRows } from "~/modules/bitcraft/gamestate/rerecipe";

export default defineEventHandler((event) => {
  const rows = getCraftingRecipesFromRows();

  return rows;
});
