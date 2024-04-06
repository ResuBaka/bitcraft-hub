import { loadItems } from "~/logic";

export default defineNitroPlugin(async (nitroApp) => {
  await loadItems();
});
