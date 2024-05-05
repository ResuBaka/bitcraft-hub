import {
  getItemRowsFromRows,
  readItemRows,
} from "~/modules/bitcraft/gamestate/item";

export default defineEventHandler((event) => {
  let { tag, tier, search, page, perPage } = getQuery(event);

  const rows = getItemRowsFromRows(readItemRows());

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
  } else {
    perPage = 16;
  }

  const rowsFilterted =
    rows?.filter((item: any) => {
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
    total: rowsFilterted.length,
    tags: Array.from(new Set(rows.map((item: any) => item.tag))),
    tiers: Array.from(new Set(rows.map((item: any) => parseInt(item.tier)))),
    page,
    perPage,
  };
});
