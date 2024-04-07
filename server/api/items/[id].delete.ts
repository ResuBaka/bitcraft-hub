import { getItem, deleteItem } from "~/logic";

export default defineEventHandler(async (event) => {
  const id = getRouterParam(event, "id", { decode: true });

  if (!id) {
    throw createError({
      statusCode: 400,
      statusMessage: "Missing ID",
    });
  }

  const item = getItem(id);

  if (!item) {
    throw createError({
      statusCode: 404,
      statusMessage: "Item was not found",
    });
  }

  await deleteItem(id);

  setResponseStatus(event, 200, "Item Deleted");
});
