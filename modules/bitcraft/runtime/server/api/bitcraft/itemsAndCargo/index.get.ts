import {
  getItemRowsFromRows,
  type ItemRow,
} from "~/modules/bitcraft/gamestate/item";

import {
  getCargoDescRowsFromRows,
  type CargoDescRow,
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

  const itemRows = getItemRowsFromRows();
  const cargoDescRows = getCargoDescRowsFromRows();

  const tagsFromItems = Array.from(
    new Set(itemRows.map((item: any) => item.tag)),
  );
  const tiersFromItems = Array.from(
    new Set(itemRows.map((item: any) => parseInt(item.tier))),
  );

  const tagsFromCargo = Array.from(
    new Set(cargoDescRows.map((item: any) => item.tag)),
  );
  const tiersFromCargo = Array.from(
    new Set(cargoDescRows.map((item: any) => parseInt(item.tier))),
  );

  const tags = Array.from(new Set([...tagsFromItems, ...tagsFromCargo]));
  const tiers = Array.from(new Set([...tiersFromItems, ...tiersFromCargo]));

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

  const rowsFilterted: ItemRow[] | CargoDescRow[] =
    itemRows?.filter((item) => {
      return (
        (!tag || item.tag === tag) &&
        (!tier || item.tier === tier) &&
        (!search ||
          item.name.toLowerCase().includes(search.toLowerCase()) ||
          !search ||
          item.id.toString().includes(search))
      );
    }) ?? [];

  const cargoRowsFilterted: CargoDescRow[] =
    cargoDescRows?.filter((item) => {
      return (
        (!tag || item.tag === tag) &&
        (!tier || item.tier === tier) &&
        (!search ||
          item.name.toLowerCase().includes(search.toLowerCase()) ||
          !search ||
          item.id.toString().includes(search))
      );
    }) ?? [];

  rowsFilterted.push(...cargoRowsFilterted);

  return {
    items: rowsFilterted.slice((page - 1) * perPage, page * perPage),
    tags: tags.sort((a, b) => a.localeCompare(b)),
    tiers: tiers.sort((a, b) => a - b),
    total: rowsFilterted.length,
    page,
    pages: Math.ceil(rowsFilterted.length / perPage),
    perPage,
  };
});
