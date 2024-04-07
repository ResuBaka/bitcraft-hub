import { getProfession, addProfession } from "~/logic";
import type { Profession } from "~/types";
import { zodNpc } from "~/logic/validations";

export default defineEventHandler(async (event) => {
  const id = getRouterParam(event, "id");
  const body = await readBody<Profession>(event);

  if (!id) {
    throw createError({
      statusCode: 400,
      statusMessage: "ID is required",
    });
  }

  const profession = getProfession(id);

  if (profession) {
    throw createError({
      statusCode: 400,
      statusMessage: "There is already a profession with that id",
    });
  }

  const result = zodNpc.safeParse(body);

  if (!result.success) {
    console.log("Profession Creation", result.error.format());
    throw createError({
      statusCode: 400,
      statusMessage: "Body is invalid",
    });
  }

  await addProfession(result.data);

  setResponseStatus(event, 200, "Profession updated");
});
