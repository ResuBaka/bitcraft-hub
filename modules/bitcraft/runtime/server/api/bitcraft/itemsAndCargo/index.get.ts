import {
  getItemRowsFromRows,
  type ItemRow,
  readItemRows,
} from "~/modules/bitcraft/gamestate/item";

import {
  getCargoDescRowsFromRows,
  readCargoDescRows,
} from "~/modules/bitcraft/gamestate/cargoDesc";

let perPageDefault = 24;
let perPageMax = perPageDefault * 4;

export type ItemQuery = {
  search?: string;
  tag?: string;
  tier?: number;
  page?: number;
  perPage?: number;
};

export type ItemResponse = {
  items: ItemRow[];
  tags: string[];
  tiers: number[];
  total: number;
  page: number;
  pages: number;
  perPage: number;
};

export default defineEventHandler<ItemResponse>((event) => {
  let { tag, tier, search, page, perPage } = getQuery<ItemQuery>(event);

  const rows1 = getItemRowsFromRows(readItemRows());
  const rows2 = getCargoDescRowsFromRows(readCargoDescRows());
  const rows = [...rows1, ...rows2];
  if (tier) {
    tier = parseInt(tier);
  }

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

  const rowsFilterted =
    rows?.filter((item) => {
      return (
        (!tag || item.tag === tag) &&
        (!tier || item.tier === tier) &&
        (!search ||
          item.name.toLowerCase().includes(search.toLowerCase()) ||
          !search ||
          item.id.toString().includes(search))
      );
    }) ?? [];

  return {
    items: rowsFilterted.slice((page - 1) * perPage, page * perPage),
    tags: Array.from(new Set(rows.map((item: any) => item.tag))),
    tiers: Array.from(new Set(rows.map((item: any) => parseInt(item.tier)))),
    total: rowsFilterted.length,
    page,
    pages: Math.ceil(rowsFilterted.length / perPage),
    perPage,
  };
});
