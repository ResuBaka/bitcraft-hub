import {
  getInventoryRowsFromRows,
  readInventoryRows,
} from "~/modules/bitcraft/gamestate/inventory";

export default defineEventHandler((event) => {
  let { search, page, owner_entity_id } = getQuery(event);

  let rows = getInventoryRowsFromRows(readInventoryRows());

  const perPage = 30;
  if (owner_entity_id) {
    owner_entity_id = parseInt(owner_entity_id);
  }

  if (page) {
    page = parseInt(page);
  } else {
    page = 1;
  }

  const rowsFilterted =
    rows?.filter((building: any) => {
      return (
        (!owner_entity_id || building.owner_entity_id === owner_entity_id) &&
        (!search ||
          building.name.toLowerCase().includes(search.toLowerCase()) ||
          !search ||
          building.entity_id.toString().includes(search))
      );
    }) ?? [];

  return {
    buildings: rowsFilterted.slice((page - 1) * perPage, page * perPage),
    total: rowsFilterted.length,
    page,
    perPage,
  };
});
