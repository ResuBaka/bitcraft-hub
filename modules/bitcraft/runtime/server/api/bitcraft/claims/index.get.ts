import {
  getClaimDescriptionRowsFromRows,
  readClaimRows,
} from "~/modules/bitcraft/gamestate/claimDescription";

interface ClaimMember {
  user_name: string;
  inventory_permission: boolean;
  build_permission: boolean;
  officer_permission: boolean;
  co_owner_permission: boolean;
}
interface ClaimDescriptionRow {
  owner_player_entity_id: number;
  owner_building_entity_id: number;
  name: string;
  supplies: number;
  building_maintenance: number;
  members: any;
  tiles: number;
  extensions: number;
  neutral: boolean;
  location: any;
  treasury: number;
}

let perPageDefault = 24;
let perPageMax = perPageDefault * 4;

export default defineEventHandler((event) => {
  let { search, page, perPage, showEmptySupplies } = getQuery(event);

  const rows = getClaimDescriptionRowsFromRows(readClaimRows());

  if (page) {
    page = parseInt(page);
  } else {
    page = 1;
  }

  if (showEmptySupplies) {
    showEmptySupplies = showEmptySupplies === "true";
  } else {
    showEmptySupplies = false;
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
    rows?.filter((item: any) => {
      if (!showEmptySupplies && item.supplies === 0) {
        return false;
      }

      return !search || item.name.toLowerCase().includes(search.toLowerCase());
    }) ?? [];

  return {
    claims: rowsFilterted.slice((page - 1) * perPage, page * perPage),
    total: rowsFilterted.length,
    page,
    perPage,
  };
});
