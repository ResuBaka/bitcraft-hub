import { items } from "~/logic";

export default defineEventHandler(async (event) => {
  return items;
});
