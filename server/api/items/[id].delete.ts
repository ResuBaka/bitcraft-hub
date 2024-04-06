import { deleteItem } from "~/logic";

export default defineEventHandler(async (event) => {
  const id = getRouterParam(event, "id");

  if (!id) {
    setResponseStatus(event, 400);
    return;
  }

  await deleteItem(id);

  setResponseStatus(event, 204);
});
