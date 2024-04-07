import { getItem, updateItem } from "~/logic";
import { z } from "zod";
import type { Item } from "~/types";
import { zodItem } from "~/logic/validations";

export default defineEventHandler(async (event) => {
  const id = getRouterParam(event, "id");
  const body = await readBody<Item>(event);

  if (!id) {
    throw createError({
      statusCode: 400,
      statusMessage: "ID is required",
    });
  }

  const item = getItem(id);

  if (!item) {
    throw createError({
      statusCode: 404,
      statusMessage: "Item not found",
    });
  }

  const result = zodItem.safeParse(body);

  if (!result.success) {
    console.log("Item Creation", result.error.format());
    throw createError({
      statusCode: 400,
      statusMessage: "Body is invalid",
    });
  }

  updateItem(result.data);

  setResponseStatus(event, 200, "Item updated");
});
