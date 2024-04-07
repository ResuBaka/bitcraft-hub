import {
  getBuilding,
  deleteBuilding,
  getBuildingRequirement,
  deleteBuildingRequirement,
} from "~/logic";

export default defineEventHandler(async (event) => {
  const id = getRouterParam(event, "id", { decode: true });
  const requirementId = getRouterParam(event, "requirementId", {
    decode: true,
  });

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
      statusMessage: "Building was not found",
    });
  }

  const requirement = getBuildingRequirement(id, requirementId);

  if (!requirement) {
    throw createError({
      statusCode: 404,
      statusMessage: "Requirement was not found",
    });
  }

  await deleteBuildingRequirement(id, requirementId);

  setResponseStatus(event, 200, "Building Deleted");
});
