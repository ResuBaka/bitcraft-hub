import {
  getTradingOrderStateRowsFromRows,
  readTradeOrderStateRows,
} from "~/modules/bitcraft/gamestate/tradeOrder";

export default defineEventHandler((event) => {
  let { search, page } = getQuery(event);

  const rows = getTradingOrderStateRowsFromRows(readTradeOrderStateRows());

  const perPage = 30;

  if (page) {
    page = parseInt(page);
  } else {
    page = 1;
  }

  const rowsFilterted =
    rows?.filter((item: any) => {
      return !search || item.name.toLowerCase().includes(search.toLowerCase());
    }) ?? [];

  return {
    claims: rowsFilterted.slice((page - 1) * perPage, page * perPage),
    total: rowsFilterted.length,
    page,
    perPage,
  };
});
