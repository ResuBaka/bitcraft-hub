import { buildings } from "~/logic";

export default defineEventHandler(async (event) => {
  return buildings;
});
