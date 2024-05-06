import { getCargoDescRowsFromRows, readCargoDescRows } from "~/modules/bitcraft/gamestate/cargoDesc";
import { getItemRowsFromRows, readItemRows } from "~/modules/bitcraft/gamestate/item";
import {
  getTradingOrderStateRowsFromRows,
  readTradeOrderStateRows,
  replaceTradeOrderItemIdWithItem,
  replaceTradeOrdersCargoIdWithCargo,
  replaceTradeOrdersItemIdWithItem,
} from "~/modules/bitcraft/gamestate/tradeOrder";
const items = getItemRowsFromRows(readItemRows());
const cargo_rows = getCargoDescRowsFromRows(readCargoDescRows());
const rows = replaceTradeOrdersItemIdWithItem(
  replaceTradeOrdersCargoIdWithCargo(getTradingOrderStateRowsFromRows(readTradeOrderStateRows()),cargo_rows), items);

export default defineEventHandler((event) => {
  let { search, page, perPage } = getQuery(event);

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
  const rowsFilterted = rows.filter(
    (trade_order) => !search || trade_order.offer_items.filter((item) => {
        return item.item.name.toLowerCase().includes(search.toLowerCase());
      }).length > 0 || trade_order.required_items.filter((item) => {
        return item.item.name.toLowerCase().includes(search.toLowerCase());
      }).length > 0  || trade_order.offer_cargo.filter((cargo) => {
        return cargo.name.toLowerCase().includes(search.toLowerCase());
      }).length > 0 || trade_order.required_cargo.filter((cargo) => {
        return cargo.name.toLowerCase().includes(search.toLowerCase());
      }).length > 0 
  );
  return {
    trade_orders: rowsFilterted.slice((page - 1) * perPage, page * perPage),
    total: rowsFilterted.length,
    page,
    perPage,
  };
});
