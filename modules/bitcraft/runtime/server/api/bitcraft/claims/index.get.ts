import { getClaimDescriptionRowsFromRows } from "~/modules/bitcraft/gamestate/claimDescription";
import { getClaimTechStates } from "~/modules/bitcraft/gamestate/claimTechState";
import { getClaimTechDescs } from "~/modules/bitcraft/gamestate/claimTechDesc";

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
  members: any[];
  tiles: number;
  extensions: number;
  neutral: boolean;
  location: any;
  treasury: number;
}

let perPageDefault = 24;
let perPageMax = perPageDefault * 6;

export default defineEventHandler((event) => {
  let { search, page, perPage, ses } = getQuery(event);

  const rows = getClaimDescriptionRowsFromRows();

  if (page) {
    page = parseInt(page);
  } else {
    page = 1;
  }

  if (ses) {
    ses = ses === "true";
  } else {
    ses = false;
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
      if (!ses && item.supplies === 0) {
        return false;
      }

      if (item.name === "Watchtower") {
        return false;
      }

      if (ses && item.supplies > 0) {
        return false;
      }

      return !search || item.name.toLowerCase().includes(search.toLowerCase());
    }) ?? [];

  const claimTechStates = getClaimTechStates();
  const claimTechDescs = getClaimTechDescs();

  const tierUpgrades = claimTechDescs.filter((desc) =>
    desc.description.startsWith("Tier "),
  );
  const tierUpgradesIds = tierUpgrades.map((desc) => desc.id);

  let claims = [...rowsFilterted];

  for (const claim of claims) {
    const claimTechState = claimTechStates.find(
      (state) => state.entity_id === claim.entity_id,
    );

    let tier = 1;
    if (claimTechState) {
      const foundTiers = claimTechState.learned.filter((id) =>
        tierUpgradesIds.includes(id),
      );
      if (foundTiers.length) {
        tier =
          tierUpgrades.find(
            (desc) => desc.id === foundTiers[foundTiers.length - 1],
          )?.tier ?? 1;
      }
    }

    claim.running_upgrade = claimTechState
      ? tierUpgrades.find((desc) => desc.id === claimTechState.researching)
      : null;
    claim.tier = tier;
    claim.upgrades =
      claimTechState?.learned.map((id) =>
        claimTechDescs.find((desc) => desc.id === id),
      ) ?? [];
  }

  claims.sort((a, b) => b.tier - a.tier);

  claims = [...claims.slice((page - 1) * perPage, page * perPage)];

  return {
    claims,
    total: rowsFilterted.length,
    page,
    perPage,
  };
});
