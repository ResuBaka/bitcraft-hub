import { getNpc, addNpc } from "~/logic";
import type { Npc } from "~/types";
import { zodNpc } from "~/logic/validations";

export default defineEventHandler(async (event) => {
  const id = getRouterParam(event, "id", { decode: true });
  const body = await readBody<Npc>(event);

  if (!id) {
    throw createError({
      statusCode: 400,
      statusMessage: "ID is required",
    });
  }

  const npc = getNpc(id);

  if (npc) {
    throw createError({
      statusCode: 400,
      statusMessage: "There is already an npc with that id",
    });
  }

  const result = zodNpc.safeParse(body);

  if (!result.success) {
    console.log("Npc Creation", result.error.format());
    throw createError({
      statusCode: 400,
      statusMessage: "Body is invalid",
    });
  }

  await addNpc(result.data);

  setResponseStatus(event, 200, "Npc updated");
});
