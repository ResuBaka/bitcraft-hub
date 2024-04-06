import { loadBuildings } from "~/logic";

export default defineNitroPlugin(async (nitroApp) => {
  await loadBuildings();
});
