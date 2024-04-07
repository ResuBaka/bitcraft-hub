import { getBuilding, addBuilding } from "~/logic";
import type { Building } from "~/types";
import { zodBuilding } from "~/logic/validations";

export default defineEventHandler(async (event) => {
  const body = await readBody<Building>(event);

  const result = zodBuilding.safeParse(body);

  if (!result.success) {
    console.log("Building Creation", result.error.format());
    throw createError({
      statusCode: 400,
      statusMessage: "Body is invalid",
    });
  }

  const building = getBuilding(result.data.id);

  if (building) {
    throw createError({
      statusCode: 400,
      statusMessage: "There is already an building with that id",
    });
  }

  await addBuilding(result.data);

  setResponseStatus(event, 200, "Building updated");
});
