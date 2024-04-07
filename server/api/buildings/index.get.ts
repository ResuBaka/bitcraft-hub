import { data } from "~/logic";

export default defineEventHandler(async (event) => {
  return data.buildings;
});
