import {
  getTradingOrderStateRowsFromRows,
  readTradeOrderStateRows,
  replaceTradeOrderItemIdWithItem,
  replaceTradeOrdersCargoIdWithCargo,
  replaceTradeOrdersItemIdWithItem,
} from "~/modules/bitcraft/gamestate/tradeOrder";

export default defineEventHandler((event) => {
  let { search, page, perPage } = getQuery(event);

  const rows = getTradingOrderStateRowsFromRows(readTradeOrderStateRows());

  if (page) {
    page = parseInt(page);
  } else {
    page = 1;
  }
  if (perPage) {
    perPage = parseInt(perPage);
  } else {
    perPage = 16;
  }

  const rowsFilterted =
    rows?.filter((item: any) => {
      return !search || item.name.toLowerCase().includes(search.toLowerCase());
    }) ?? [];

  return {
    trade_orders: replaceTradeOrdersItemIdWithItem(
      replaceTradeOrdersCargoIdWithCargo(
        rowsFilterted.slice((page - 1) * perPage, page * perPage),
      ),
    ),
    total: rowsFilterted.length,
    page,
    perPage,
  };
});
