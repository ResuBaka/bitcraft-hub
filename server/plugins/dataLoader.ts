import { loadRoot } from "~/logic";


export default defineNitroPlugin(async (nitroApp) => {
  await loadRoot();
});
