import { getItem, addItem } from "~/logic";
import { z } from "zod";
import type { Item } from "~/types";
import { zodItem } from "~/logic/validations";

export default defineEventHandler(async (event) => {
  const id = getRouterParam(event, "id", { decode: true });
  const body = await readBody<Item>(event);

  if (!id) {
    throw createError({
      statusCode: 400,
      statusMessage: "ID is required",
    });
  }

  const item = getItem(id);

  if (item) {
    throw createError({
      statusCode: 400,
      statusMessage: "There is already an item with that id",
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

  await addItem(result.data);

  setResponseStatus(event, 200, "Item updated");
});
