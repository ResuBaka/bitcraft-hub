import {
  getCargoDescRowsFromRows,
  readCargoDescRows,
} from "~/modules/bitcraft/gamestate/cargoDesc";
import {
  getItemRowsFromRows,
  readItemRows,
} from "~/modules/bitcraft/gamestate/item";
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
  replaceTradeOrdersCargoIdWithCargo(
    getTradingOrderStateRowsFromRows(readTradeOrderStateRows()),
    cargo_rows,
  ),
  items,
);

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
  const searchLowerCase = search.toLowerCase();

  const rowsFilterted = rows.filter(
    ({ offer_items, required_items, offer_cargo, required_cargo }) =>
      !search ||
      offer_items.filter(({ item }) => {
        return item.name.toLowerCase().includes(searchLowerCase);
      }).length > 0 ||
      required_items.filter(({ item }) => {
        return item.name.toLowerCase().includes(searchLowerCase);
      }).length > 0 ||
      offer_cargo.filter(({ name }) => {
        return name.toLowerCase().includes(searchLowerCase);
      }).length > 0 ||
      required_cargo.filter(({ name }) => {
        return name.toLowerCase().includes(searchLowerCase);
      }).length > 0,
  );
  return {
    trade_orders: rowsFilterted.slice((page - 1) * perPage, page * perPage),
    total: rowsFilterted.length,
    page,
    perPage,
  };
});
