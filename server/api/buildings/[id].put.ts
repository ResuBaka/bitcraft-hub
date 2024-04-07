import { getBuilding, addBuilding } from "~/logic";
import type { Building } from "~/types";
import { zodBuilding } from "~/logic/validations";

export default defineEventHandler(async (event) => {
  const id = getRouterParam(event, "id", { decode: true });
  const body = await readBody<Building>(event);

  if (!id) {
    throw createError({
      statusCode: 400,
      statusMessage: "ID is required",
    });
  }

  const building = getBuilding(id);

  if (building) {
    throw createError({
      statusCode: 400,
      statusMessage: "There is already an building with that id",
    });
  }

  const result = zodBuilding.safeParse(body);

  if (!result.success) {
    console.log("Building Creation", result.error.format());
    throw createError({
      statusCode: 400,
      statusMessage: "Body is invalid",
    });
  }

  await addBuilding(result.data);

  setResponseStatus(event, 200, "Building updated");
});
