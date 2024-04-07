import { getProfession, addProfession } from "~/logic";
import type { Profession } from "~/types";
import { zodProfession } from "~/logic/validations";

export default defineEventHandler(async (event) => {
  const body = await readBody<Profession>(event);

  const result = zodProfession.safeParse(body);

  if (!result.success) {
    console.log("Profession Creation", result.error.format());
    throw createError({
      statusCode: 400,
      statusMessage: "Body is invalid",
    });
  }

  const profession = getProfession(result.data.id);

  if (profession) {
    throw createError({
      statusCode: 400,
      statusMessage: "There is already a profession with that id",
    });
  }

  await addProfession(result.data);

  setResponseStatus(event, 201, "Profession Created");
});
