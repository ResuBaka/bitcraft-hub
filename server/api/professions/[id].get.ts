import { getProfession } from "~/logic";

export default defineEventHandler(async (event) => {
  const id = getRouterParam(event, "id");

  if (!id) {
    throw createError({
      statusCode: 400,
      statusMessage: "Missing ID",
    });
  }

  const profession = getProfession(id);

  if (!profession) {
    throw createError({
      statusCode: 404,
      statusMessage: "Profession was not found",
    });
  }

  return profession;
});
