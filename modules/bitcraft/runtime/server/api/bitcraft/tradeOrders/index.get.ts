import { getCargoDescRowsFromRows } from "~/modules/bitcraft/gamestate/cargoDesc";
import { getItemRowsFromRows } from "~/modules/bitcraft/gamestate/item";
import {
  getTradingOrderStateRowsFromRows,
  replaceTradeOrdersCargoIdWithCargo,
  replaceTradeOrdersItemIdWithItem,
  type TradingOrderStateRow,
} from "~/modules/bitcraft/gamestate/tradeOrder";

let perPageDefault = 24;
let perPageMax = perPageDefault * 4;

export type TradeOrderQuery = {
  search?: string;
  page?: number;
  perPage?: number;
};

export type TradeOrderResponse = {
  trade_orders: TradingOrderStateRow[];
  total: number;
  page: number;
  perPage: number;
};

export default defineEventHandler<TradeOrderResponse>((event) => {
  let { search, page, perPage } = getQuery<TradeOrderQuery>(event);

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

  const items = getItemRowsFromRows();
  const cargo_rows = getCargoDescRowsFromRows();
  const rows = replaceTradeOrdersItemIdWithItem(
    replaceTradeOrdersCargoIdWithCargo(
      getTradingOrderStateRowsFromRows(),
      cargo_rows,
    ),
    items,
  );

  const searchLowerCase = search?.toLowerCase();

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
