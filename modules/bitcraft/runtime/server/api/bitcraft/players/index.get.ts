import { getPlayerRowsFromRows } from "~/modules/bitcraft/gamestate/player";

let perPageDefault = 24;
let perPageMax = perPageDefault * 6;

export default defineEventHandler((event) => {
  let { search, page, perPage } = getQuery(event);

  if (page) {
    page = parseInt(page);
  } else {
    page = 1;
  }

  if (perPage) {
    perPage = parseInt(perPage);
    if (perPage > perPageMax) {
      perPage = perPageDefault;
    }
  } else {
    perPage = perPageDefault;
  }

  const rows = getPlayerRowsFromRows();
  const rowsFilterted =
    rows?.filter((player) => {
      return (
        !search ||
        player.username.toLowerCase().includes(search.toLowerCase()) ||
        !search ||
        player.entity_id.toString().includes(search)
      );
    }) ?? [];

  return {
    players: rowsFilterted.slice((page - 1) * perPage, page * perPage),
    total: rowsFilterted.length,
    page,
    perPage,
  };
});
