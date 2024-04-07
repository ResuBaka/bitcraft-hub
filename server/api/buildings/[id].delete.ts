import { getBuilding, deleteBuilding } from "~/logic";

export default defineEventHandler(async (event) => {
  const id = getRouterParam(event, "id");

  if (!id) {
    throw createError({
      statusCode: 400,
      statusMessage: "Missing ID",
    });
  }

  const building = getBuilding(id);

  if (!building) {
    throw createError({
      statusCode: 404,
      statusMessage: "Building was not found",
    });
  }

  await deleteBuilding(id);

  setResponseStatus(event, 200, "Building Deleted");
});
