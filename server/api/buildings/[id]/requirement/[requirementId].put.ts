import {
  addBuildingRequirement,
  getBuilding,
  getBuildingRequirement,
} from "~/logic";
import type { Building } from "~/types";
import { zodRequirement } from "~/logic/validations";

export default defineEventHandler(async (event) => {
  const id = getRouterParam(event, "id", { decode: true });
  const requirementId = getRouterParam(event, "requirementId", {
    decode: true,
  });

  const body = await readBody<Building>(event);

  if (!id) {
    throw createError({
      statusCode: 400,
      statusMessage: "Missing building ID",
    });
  }

  if (!requirementId) {
    throw createError({
      statusCode: 400,
      statusMessage: "Missing requirement ID",
    });
  }

  const building = getBuilding(id);

  if (!building) {
    throw createError({
      statusCode: 404,
      statusMessage: "Building not found",
    });
  }

  const result = zodRequirement.safeParse(body);

  if (!result.success) {
    console.log("Building Creation", result.error.format());
    throw createError({
      statusCode: 400,
      statusMessage: "Body is invalid",
    });
  }

  await addBuildingRequirement(id, result.data);

  setResponseStatus(event, 200, "Building updated");
});
