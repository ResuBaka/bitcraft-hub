import {
  getEmpireStateRowsFromRows,
  readEmpireState,
} from "~/modules/bitcraft/gamestate/empireState";

let perPageDefault = 24;
let perPageMax = perPageDefault * 4;

export default defineEventHandler((event) => {
  let { search, page, perPage, showEmptySupplies } = getQuery(event);

  const rows = getEmpireStateRowsFromRows(readEmpireState());

  if (page) {
    page = parseInt(page);
  } else {
    page = 1;
  }

  if (showEmptySupplies) {
    showEmptySupplies = showEmptySupplies === "true";
  } else {
    showEmptySupplies = false;
  }

  if (perPage) {
    perPage = parseInt(perPage);
    if (perPage > perPageMax) {
      perPage = perPageDefault;
    }
  } else {
    perPage = perPageDefault;
  }

  const rowsFilterted =
    rows?.filter((item: any) => {
      if (!showEmptySupplies && item.supplies === 0) {
        return false;
      }

      return !search || item.name.toLowerCase().includes(search.toLowerCase());
    }) ?? [];

  return {
    claims: rowsFilterted.slice((page - 1) * perPage, page * perPage),
    total: rowsFilterted.length,
    page,
    perPage,
  };
});
