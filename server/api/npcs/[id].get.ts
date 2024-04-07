import { getNpc } from "~/logic";

export default defineEventHandler(async (event) => {
  const id = getRouterParam(event, "id");

  if (!id) {
    throw createError({
      statusCode: 400,
      statusMessage: "Missing ID",
    });
  }

  const npc = getNpc(id);

  if (!npc) {
    throw createError({
      statusCode: 404,
      statusMessage: "Npc was not found",
    });
  }

  return npc;
});
