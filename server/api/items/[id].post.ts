import { getItem, updateItem } from "~/logic";
import { z } from "zod";

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

  const result = z
    .object({
      title: z.string(),
      id: z.string(),
      tier: z.number(),
      building: z.string().optional(),
      tool: z.string().optional(),
      skill: z.string().optional(),
      creates: z.number().default(1),
      items: z.array(
        z.object({
          id: z.string(),
          amount: z.number(),
        }),
      ),
    })
    .safeParse(body);

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
