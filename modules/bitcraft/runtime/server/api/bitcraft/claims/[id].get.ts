import {
  type ClaimDescriptionRow,
  getClaimDescriptionRowsFromRows,
} from "~/modules/bitcraft/gamestate/claimDescription";
import { getBuildingStateRowsFromRows } from "~/modules/bitcraft/gamestate/buildingState";
import { getBuildingDescIdMapFromRows } from "~/modules/bitcraft/gamestate/buildingDesc";
import {
  getInventorys,
  replaceInventoryItemIdWithItem,
  replaceInventoryItemsIdWithItems,
} from "~/modules/bitcraft/gamestate/inventory";
import { getItemRowsFromRows } from "~/modules/bitcraft/gamestate/item";
import {
  getCagoDescFromCargoId,
  getCargoDescRowsFromRows,
} from "~/modules/bitcraft/gamestate/cargoDesc";
import { getClaimTechStates } from "~/modules/bitcraft/gamestate/claimTechState";
import { getClaimTechDescs } from "~/modules/bitcraft/gamestate/claimTechDesc";
import { getPlayerRowsFromRows } from "~/modules/bitcraft/gamestate/player";

export default defineEventHandler((event) => {
  const rows = getClaimDescriptionRowsFromRows();
  const id = getRouterParam(event, "id", { decode: true });

  if (!id) {
    throw createError({
      statusCode: 400,
      statusMessage: "Missing ID",
    });
  }

  const claim = rows.find((claims) => claims.entity_id == parseInt(id));

  if (!claim) {
    throw createError({
      statusCode: 404,
      statusMessage: "Claim was not found",
    });
  }

  const claimTechStates = getClaimTechStates();
  const claimTechDescs = getClaimTechDescs();

  const tierUpgrades = claimTechDescs.filter((desc) =>
    desc.description.startsWith("Tier "),
  );
  const tierUpgradesIds = tierUpgrades.map((desc) => desc.id);

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

  const a = getInventorysFromClaimMerged(claim);

  return {
    ...claim,
    inventorys: {
      buildings: a.buildings,
      players: a.players,
      players_offline: a.players_offline,
    },
    time_played: a.time_played,
  };
});

function getInventorysFromClaimMerged(claim: ClaimDescriptionRow) {
  let rows = getBuildingStateRowsFromRows();

  const buildingDescMap = getBuildingDescIdMapFromRows();
  rows = rows.filter((buildingState) => {
    const buildingDesc = buildingDescMap.get(
      buildingState.building_description_id,
    );

    buildingState.building_name = buildingDesc?.name;
    buildingState.image_path = buildingDesc?.icon_asset_name + ".png";

    if (buildingDesc === undefined) {
      return false;
    }
    if (buildingDesc.name.includes("Chest")) {
      return true;
    }
    if (buildingDesc.name.includes("Stockpile")) {
      return true;
    }
    return false;
  });

  const rowsFilterted =
    rows?.filter((building: any) => {
      return building.claim_entity_id === claim.entity_id;
    }) ?? [];

  const buildingIds = rowsFilterted.map((building) => building.entity_id);
  const itemsTemp = getItemRowsFromRows();
  const cargo_rows = getCargoDescRowsFromRows();
  const inventorys = getInventorys();

  const rowsInventory = replaceInventoryItemsIdWithItems(
    inventorys.filter((inventory) =>
      buildingIds.includes(inventory.owner_entity_id),
    ) ?? [],
    itemsTemp,
  );

  const playerRowsInventoryNoneMapped = [];
  const playerOfflineRowsInventoryNoneMapped = [];
  const playerRows = getPlayerRowsFromRows();

  let time_played = 0;

  for (const member of claim.members) {
    const player = playerRows.find(
      (player) => player.entity_id === member.entity_id,
    );

    if (player) {
      time_played += player.time_played;
    }

    if (player === undefined || !player.signed_in) {
      const playersInventory = inventorys.filter(
        (inventory) => inventory.owner_entity_id === member.entity_id,
      );

      const playersInventorySorted = playersInventory.sort(
        (a, b) => a.entity_id - b.entity_id,
      );

      if (playersInventorySorted.length > 0) {
        for (const inventory of playersInventorySorted.length > 1
          ? playersInventorySorted.slice(1)
          : playersInventorySorted) {
          playerOfflineRowsInventoryNoneMapped.push(inventory);
        }
      }
    } else {
      const playersInventory = inventorys.filter(
        (inventory) => inventory.owner_entity_id === member.entity_id,
      );

      const playersInventorySorted = playersInventory.sort(
        (a, b) => a.entity_id - b.entity_id,
      );

      if (playersInventorySorted.length > 0) {
        for (const inventory of playersInventorySorted.length > 1
          ? playersInventorySorted.slice(1)
          : playersInventorySorted) {
          playerRowsInventoryNoneMapped.push(inventory);
        }
      }
    }
  }

  let items = {};

  for (const inventory of rowsInventory) {
    for (const pocket of inventory.pockets) {
      if (pocket.contents !== undefined) {
        if (
          pocket.contents.item_type === "Cargo" &&
          items[pocket.contents.item_id] === undefined
        ) {
          items[pocket.contents.item_id] = {
            ...pocket.contents,
            item: getCagoDescFromCargoId(cargo_rows, pocket.contents.item_id),
          };

          continue;
        } else if (pocket.contents.item_type === "Cargo") {
          items[pocket.contents.item_id].quantity += pocket.contents.quantity;
          continue;
        }

        if (
          pocket.contents.item_type === "Item" &&
          items[pocket.contents.item_id] === undefined
        ) {
          items[pocket.contents.item_id] = { ...pocket.contents };
          continue;
        } else if (pocket.contents.item_type === "Item") {
          items[pocket.contents.item_id].quantity += pocket.contents.quantity;
          continue;
        }
      }
    }
  }

  const playerRowsInventory = replaceInventoryItemsIdWithItems(
    playerRowsInventoryNoneMapped,
    itemsTemp,
  );

  let itemsPlayer = {};
  for (const inventory of playerRowsInventory) {
    for (const pocket of inventory.pockets) {
      if (pocket.contents !== undefined) {
        if (
          pocket.contents.item_type === "Cargo" &&
          itemsPlayer[pocket.contents.item_id] === undefined
        ) {
          itemsPlayer[pocket.contents.item_id] = {
            ...pocket.contents,
            item: getCagoDescFromCargoId(cargo_rows, pocket.contents.item_id),
          };

          continue;
        } else if (pocket.contents.item_type === "Cargo") {
          itemsPlayer[pocket.contents.item_id].quantity +=
            pocket.contents.quantity;
          continue;
        }

        if (
          pocket.contents.item_type === "Item" &&
          itemsPlayer[pocket.contents.item_id] === undefined
        ) {
          itemsPlayer[pocket.contents.item_id] = { ...pocket.contents };
          continue;
        } else if (pocket.contents.item_type === "Item") {
          itemsPlayer[pocket.contents.item_id].quantity +=
            pocket.contents.quantity;
          continue;
        }
      }
    }
  }

  const playerOfflineRowsInventory = replaceInventoryItemsIdWithItems(
    playerOfflineRowsInventoryNoneMapped,
    itemsTemp,
  );

  let itemsPlayerOffline = {};
  for (const inventory of playerOfflineRowsInventory) {
    for (const pocket of inventory.pockets) {
      if (pocket.contents !== undefined) {
        if (
          pocket.contents.item_type === "Cargo" &&
          itemsPlayerOffline[pocket.contents.item_id] === undefined
        ) {
          itemsPlayerOffline[pocket.contents.item_id] = {
            ...pocket.contents,
            item: getCagoDescFromCargoId(cargo_rows, pocket.contents.item_id),
          };

          continue;
        } else if (pocket.contents.item_type === "Cargo") {
          itemsPlayerOffline[pocket.contents.item_id].quantity +=
            pocket.contents.quantity;
          continue;
        }

        if (
          pocket.contents.item_type === "Item" &&
          itemsPlayerOffline[pocket.contents.item_id] === undefined
        ) {
          itemsPlayerOffline[pocket.contents.item_id] = { ...pocket.contents };
          continue;
        } else if (pocket.contents.item_type === "Item") {
          itemsPlayerOffline[pocket.contents.item_id].quantity +=
            pocket.contents.quantity;
          continue;
        }
      }
    }
  }

  return {
    buildings: Object.values(items).sort((a, b) =>
      a.quantity > b.quantity ? -1 : 1,
    ),
    players: Object.values(itemsPlayer).sort((a, b) =>
      a.quantity > b.quantity ? -1 : 1,
    ),
    players_offline: Object.values(itemsPlayerOffline).sort((a, b) =>
      a.quantity > b.quantity ? -1 : 1,
    ),
    time_played,
  };
}
