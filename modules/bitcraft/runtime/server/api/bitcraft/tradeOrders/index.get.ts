import { getCargoDescRowsFromRows } from "~/modules/bitcraft/gamestate/cargoDesc";
import { getItemRowsFromRows } from "~/modules/bitcraft/gamestate/item";
import {
  getTradingOrderStateRowsFromRows,
  replaceTradeOrdersCargoIdWithCargo,
  replaceTradeOrdersItemIdWithItem,
  type TradingOrderStateRow,
} from "~/modules/bitcraft/gamestate/tradeOrder";
import { getBuildingStateRowsFromRows } from "~/modules/bitcraft/gamestate/buildingState";

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
  const building_rows = getBuildingStateRowsFromRows();
  const rows = replaceTradeOrdersItemIdWithItem(
    replaceTradeOrdersCargoIdWithCargo(
      getTradingOrderStateRowsFromRows(),
      cargo_rows,
    ),
    items,
  );

  const filter = (row: TradingOrderStateRow) => {
    return (
      !search ||
      row.offer_items.filter(({ item }) => {
        return item.name.toLowerCase().includes(searchLowerCase);
      }).length > 0 ||
      row.required_items.filter(({ item }) => {
        return item.name.toLowerCase().includes(searchLowerCase);
      }).length > 0 ||
      row.offer_cargo.filter(({ name }) => {
        return name.toLowerCase().includes(searchLowerCase);
      }).length > 0 ||
      row.required_cargo.filter(({ name }) => {
        return name.toLowerCase().includes(searchLowerCase);
      }).length > 0
    );
  };

  const searchLowerCase = search?.toLowerCase();

  const filterdWithInfo = [];

  for (const row of rows) {
    const building = building_rows.find(
      (building) => building.entity_id === row.shop_entity_id,
    );
    const shop_type =
      building && building.constructed_by_player_entity_id
        ? "Building"
        : "Shop";

    if (shop_type === "Shop") {
      continue;
    }

    if (!filter(row)) {
      continue;
    }

    row.shop_type = shop_type;
    row.shop_name = building.nickname ?? building.nickname;
    row.building = building;

    filterdWithInfo.push(row);
  }

  return {
    trade_orders: filterdWithInfo.slice((page - 1) * perPage, page * perPage),
    total: filterdWithInfo.length,
    page,
    perPage,
  };
});
