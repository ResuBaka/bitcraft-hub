import {
  getExperienceRowsFromRows,
  readExperienceStateRows,
} from "~/modules/bitcraft/gamestate/experienceState";
const rows = getExperienceRowsFromRows(readExperienceStateRows());

export default defineEventHandler((event) => {
  let { search, page, perPage } = getQuery(event);

  if (perPage) {
    perPage = parseInt(perPage);
  } else {
    perPage = 24;
  }

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
