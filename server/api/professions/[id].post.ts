import { getProfession, updateProfession } from "~/logic";
import type { Profession } from "~/types";
import { zodProfession } from "~/logic/validations";

export default defineEventHandler(async (event) => {
  const id = getRouterParam(event, "id", { decode: true });
  const body = await readBody<Profession>(event);

  if (!id) {
    throw createError({
      statusCode: 400,
      statusMessage: "ID is required",
    });
  }

  const profession = getProfession(id);

  if (!profession) {
    throw createError({
      statusCode: 404,
      statusMessage: "Profession not found",
    });
  }

  const result = zodProfession.safeParse(body);

  if (!result.success) {
    console.log("Profession Creation", result.error.format());
    throw createError({
      statusCode: 400,
      statusMessage: "Body is invalid",
    });
  }

  await updateProfession(result.data);

  setResponseStatus(event, 200, "Profession updated");
});
