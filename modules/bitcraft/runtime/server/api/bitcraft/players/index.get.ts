import {
  getPlayerRowsFromRows,
  readPlayerStateRows,
} from "~/modules/bitcraft/gamestate/player";

const rows = getPlayerRowsFromRows(readPlayerStateRows());

export default defineEventHandler((event) => {
  let { search, page } = getQuery(event);
  const perPage = 16;

  if (page) {
    page = parseInt(page);
  } else {
    page = 1;
  }

  const rowsFilterted =
    rows?.filter((player) => {
      return (
        (!search ||
          player.username.toLowerCase().includes(search.toLowerCase()) ||
          !search ||
          player.entity_id.toString().includes(search))
      );
    }) ?? [];

  return {
    players: rowsFilterted.slice((page - 1) * perPage, page * perPage),
    total: rowsFilterted.length,
    page,
    perPage,
  };
});
