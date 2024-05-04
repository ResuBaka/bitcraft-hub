import { getAllConsumedItems, getAllConsumedItemsFromItem, getCraftingRecipesFromRows, readCraftingRecipeRows } from "../gamestate/rerecipe";

export default async function RequestAllPlayerInfo() {
  const rows = getCraftingRecipesFromRows(readCraftingRecipeRows());
  console.log(JSON.stringify(getAllConsumedItemsFromItem(rows, 6140007)))
  
}

RequestAllPlayerInfo();
