import { loadRoot } from "~/logic";

export default defineEventHandler(async (event) => {
  if (import.meta.dev) {
    await loadRoot();
  }
});
