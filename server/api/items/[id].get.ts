import { getAllItem } from "~/logic";

export default defineEventHandler(async (event) => {
  const id = getRouterParam(event, "id");

  if (!id) {
    throw createError({
      statusCode: 400,
      statusMessage: "Missing ID",
    });
  }

  const item = getAllItem(id);

  if (!item) {
    throw createError({
      statusCode: 404,
      statusMessage: "Item not found",
    });
  }

  return item;
});
