import { addItem, items } from "~/logic";
import { z } from "zod";

export default defineEventHandler(async (event) => {
  const body = await readBody<Item>(event);

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
    console.log("Item Cration", result.error.format());
    throw createError({
      statusCode: 400,
      statusMessage: "Body is invalid",
    });
  }

  await addItem(result.data);
});
