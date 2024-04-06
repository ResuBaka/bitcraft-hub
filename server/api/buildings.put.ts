import { addBuilding } from "~/logic";
import { z } from "zod";

export default defineEventHandler(async (event) => {
  const body = await readBody(event);

  const result = z
    .object({
      title: z.string(),
      id: z.string(),
      tier: z.number(),
      items_can_be_crafted: z.array(z.string()),
    })
    .safeParse(body);

  if (!result.success) {
    console.log("Body Cration", result.error.format());
    throw createError({
      statusCode: 400,
      statusMessage: "Body is invalid",
    });
  }

  await addBuilding(result.data);
});
