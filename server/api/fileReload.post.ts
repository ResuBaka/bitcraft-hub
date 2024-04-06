import { loadItems, loadBuildings } from "~/logic";

export default defineEventHandler(async (event) => {
  if (import.meta.dev) {
    await loadItems();
    await loadBuildings();
  }
});
