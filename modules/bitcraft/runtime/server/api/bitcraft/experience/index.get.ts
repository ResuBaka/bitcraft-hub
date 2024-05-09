import {
  getExperienceRowsFromRows,
  readExperienceStateRows,
} from "~/modules/bitcraft/gamestate/experienceState";
const rows = getExperienceRowsFromRows(readExperienceStateRows());

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
      return !search;
    }) ?? [];

  return {
    players: rowsFilterted.slice((page - 1) * perPage, page * perPage),
    total: rowsFilterted.length,
    page,
    perPage,
  };
});
