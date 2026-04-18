<script setup lang="ts">
import type { SortingState } from "@tanstack/vue-table";
import { getSortedRowModel } from "@tanstack/vue-table";
import { useNow } from "@vueuse/core";
import { watchThrottled } from "@vueuse/shared";

const toast = useToast();

import AutocompleteItem from "~/components/Bitcraft/autocomplete/AutocompleteItem.vue";
import AutocompleteUser from "~/components/Bitcraft/autocomplete/AutocompleteUser.vue";
import InventoryChanges from "~/components/Bitcraft/InventoryChanges.vue";
import InventoryImg from "~/components/Bitcraft/InventoryImg.vue";
import LeaderboardClaim from "~/components/Bitcraft/LeaderboardClaim.vue";
import { iconAssetUrlNameRandom } from "~/composables/iconAssetName";
import { registerWebsocketMessageHandler } from "~/composables/websocket";
import type { BuildingStatesResponse } from "~/types/BuildingStatesResponse";
import type { ClaimDescriptionStateWithInventoryAndPlayTime } from "~/types/ClaimDescriptionStateWithInventoryAndPlayTime";
import type { ClaimTechDesc } from "~/types/ClaimTechDesc";
import type { ExpendedRefrence } from "~/types/ExpendedRefrence";
import type { InventoryChangelog } from "~/types/InventoryChangelog";
import type { InventoryItemLocation } from "~/types/InventoryItemLocation";
import type { InventoryLocationEntry } from "~/types/InventoryLocationEntry";
import type { ItemCargo } from "~/types/ItemCargo";
import type { ItemsAndCargollResponse } from "~/types/ItemsAndCargollResponse";
import type { TravelerTaskDesc } from "~/types/TravelerTaskDesc";
import { levelToColor, raritySort, rarityToTextClass, tierToBorderClassByLevel } from "~/utils";
import type { BuildingDescriptionsResponse } from "~/types/BuildingDescriptionsResponse";

const {
  public: { iconDomain },
} = useRuntimeConfig();

const page = ref(1);
const buildingItemsPage = ref(1);
const playerItemsPage = ref(1);
const playerToolsPage = ref(1);
const playerOfflineItemsPage = ref(1);
const playerOfflineToolsPage = ref(1);
const inventoryPageSize = 52;
const perPage = 1500;

const search = ref<string | null>("");

const route = useRoute();
const router = useRouter();

const player_id = ref<bigint | null>();
const item_object = ref<ItemCargo | undefined>();

const locationModalOpen = ref(false);
const selectedInventoryItem = ref<ExpendedRefrence | null>(null);
const selectedInventoryLocation = ref<InventoryItemLocation | null>(null);

const rarityBuildings = ref<string | null>(null);
const tierBuildings = ref<number | null>(null);

const rarityPlayers = ref<string | null>(null);
const tierPlayers = ref<number | null>(null);

const rarityPlayersOffline = ref<string | null>(null);
const tierPlayersOffline = ref<number | null>(null);

const rarityPlayersTools = ref<string | null>(null);
const tierPlayersTools = ref<number | null>(null);

const rarityPlayersOfflineTools = ref<string | null>(null);
const tierPlayersOfflineTools = ref<number | null>(null);

const tmpPage = (route.query.page as string) ?? null;

const tierToColor = useTierColor();

const sortToolInventory = (a: any, b: any) => {
  if (a.item.tier !== b.item.tier) {
    return b.item.tier - a.item.tier;
  }

  const nameCompare = a.item.name.localeCompare(b.item.name);
  if (nameCompare !== 0) {
    return nameCompare;
  }

  return raritySort(a.item.rarity, b.item.rarity);
};

if (tmpPage) {
  page.value = parseInt(tmpPage);
}

const { data: claimFetch } = useFetchMsPack<ClaimDescriptionStateWithInventoryAndPlayTime>(
  () => {
    return `/api/bitcraft/claims/${route.params.id.toString()}`;
  },
  { deep: true },
);

const { data: buildingDescs } = useFetchMsPack<BuildingDescriptionsResponse>(() => {
  return `/api/bitcraft/desc/buildings?per_page=1000`;
});

const getBuildingIcon = (buildingDescId: number) => {
  const desc = buildingDescs.value?.buildings?.find((desc) => desc.id === buildingDescId);

  if (desc) {
    return desc.icon_asset_name;
  }

  return desc;
};

const { data: trevelerTasksFetch } = useFetchMsPack<{
  [key: number]: TravelerTaskDesc;
}>(() => {
  return `/traveler_tasks`;
});

const { data: itemsAndCargoAllFetch } = useFetchMsPack<ItemsAndCargollResponse>(() => {
  return `/api/bitcraft/itemsAndCargo/all`;
});

const travelerSortLookup = {
  rumbagh: 1,
  svim: 2,
  heimlich: 3,
  brico: 4,
  alesi: 5,
  ramparte: 6,
};

const travelerTaskRows = computed(() => {
  if (!itemsAndCargoAllFetch.value) {
    return [];
  }

  const playersMap = claimFetch.value?.traveler_tasks?.players ?? {};
  return Object.entries(playersMap)
    .map(([taskId, players]) => {
      const taskKey = Number(taskId);
      const task = trevelerTasksFetch.value?.[taskKey];
      const requiredItems = task?.required_items ?? [];
      const itemNames = requiredItems
        .map((requiredItem) => {
          if (requiredItem.item_type === "Item") {
            return itemsAndCargoAllFetch.value?.item_desc?.[requiredItem.item_id]?.name;
          }
          if (requiredItem.item_type === "Cargo") {
            return itemsAndCargoAllFetch.value?.cargo_desc?.[requiredItem.item_id]?.name;
          }
          return undefined;
        })
        .filter((name): name is string => Boolean(name));
      const npcName = task?.description?.split(" ")[0] ?? "";
      return {
        task_id: taskKey,
        players,
        items: requiredItems,
        name: itemNames.join(", "),
        npc_name: npcName,
        player_count: players.length,
      };
    })
    .sort(
      (a, b) =>
        travelerSortLookup[a.npc_name.toLocaleLowerCase()] -
        travelerSortLookup[b.npc_name.toLocaleLowerCase()],
    );
});

const { data: InventoryChangelogFetch, refresh: InventoryChangelogRefresh } = useFetchMsPack<
  InventoryChangelog[]
>(
  () => {
    return `/claims/inventory_changelog/${route.params.id.toString()}`;
  },
  {
    onRequest: ({ options }) => {
      options.query = options.query || {};
      if (item_object.value !== undefined && item_object.value !== null) {
        options.query.item_id = item_object.value.id;
        options.query.item_type = item_object.value.type;
      }
      if (player_id.value !== undefined && player_id.value !== null) {
        options.query.user_id = player_id.value.toString();
      }
      options.query.per_page = 20;

      if (Object.keys(options.query).length > 1) {
        const query = { ...options.query };
        delete query.per_page;
        router.push({ query });
      } else if (options.query.page < 1) {
        router.push({});
      }
    },
  },
);

const { data: buidlingsFetch, pending: buildingsPending } = useFetchMsPack<BuildingStatesResponse>(
  () => {
    return `/api/bitcraft/buildings?claim_entity_id=${route.params.id}&page=${page.value}&per_page=${perPage}&skip_static_buildings=true&with_inventory=true`;
  },
);

const topicsPlayer = computed<string[]>(() => {
  if (claimFetch.value === undefined) {
    return [];
  }
  return (
    Object.keys(claimFetch.value?.members).map((entity_id) => {
      return `player_state.${entity_id}`;
    }) ?? []
  );
});

registerWebsocketMessageHandler(
  "ClaimLocalState",
  [`claim_local_state.${route.params.id}`],
  (message) => {
    if (Number(message.entity_id) === Number(route.params.id)) {
      if (claimFetch.value) {
        claimFetch.value.num_tiles = message.num_tiles;
        claimFetch.value.supplies = message.supplies;
        claimFetch.value.treasury = message.treasury;
        claimFetch.value.xp_gained_since_last_coin_minting =
          message.xp_gained_since_last_coin_minting;
      }
    }
  },
);

registerWebsocketMessageHandler("PlayerState", topicsPlayer, (message) => {
  let onlineState = message.signed_in ? "Online" : "Offline";

  if (claimFetch.value.members[message.entity_id].online_state !== onlineState) {
    if (message.signed_in) {
      toast.add({
        title: `${claimFetch.value.members[message.entity_id].user_name} signed in`,
      });
    } else {
      toast.add({
        title: `${claimFetch.value.members[message.entity_id].user_name} signed out`,
      });
    }
    claimFetch.value.members[message.entity_id].online_state = onlineState;
  }
});

const topicsLevel = computed<string[]>(() => {
  if (claimFetch.value === undefined) {
    return [];
  }
  return (
    Object.values(claimFetch.value?.members)
      .filter((member) => member.online_state === "Online")
      .map((member) => {
        return `level.${member.entity_id}`;
      }) ?? []
  );
});

registerWebsocketMessageHandler("Level", topicsLevel, (message) => {
  if (claimFetch.value?.members[message.user_id]) {
    toast.add({
      title: `Player ${claimFetch.value?.members[message.user_id].user_name} Level ${message.level} reached for Skill ${message.skill_name}`,
    });

    claimFetch.value.members[message.user_id].skills_ranks[message.skill_name].level =
      message.level;
  }
});

const claim = computed(() => {
  return claimFetch.value ?? undefined;
});
const buildings = computed(() => {
  if (!buidlingsFetch.value?.buildings.length) {
    return [];
  }

  return buidlingsFetch.value?.buildings.filter((building) => {
    return (
      !search.value || building.building_name.toLowerCase().includes(search.value.toLowerCase())
    );
  });
});

const inventoryBuildingsSearch = ref<string | null>("");

const inventorysBuildings = computed(() => {
  if (!claimFetch.value?.inventorys?.buildings?.length) {
    return [];
  }

  if (!inventoryBuildingsSearch.value && !rarityBuildings.value && !tierBuildings.value) {
    return claimFetch.value?.inventorys?.buildings;
  }

  return claimFetch.value?.inventorys?.buildings.filter(
    (inventory) =>
      (!rarityBuildings.value || inventory.item.rarity === rarityBuildings.value) &&
      (!tierBuildings.value || inventory.item.tier === tierBuildings.value) &&
      (!inventoryBuildingsSearch.value ||
        inventory.item.name.toLowerCase().includes(inventoryBuildingsSearch.value.toLowerCase())),
  );
});

watch([rarityBuildings, tierBuildings], () => {
  buildingItemsPage.value = 1;
});

const pagedInventoryBuildings = computed(() => {
  const start = (buildingItemsPage.value - 1) * inventoryPageSize;
  return inventorysBuildings.value.slice(start, start + inventoryPageSize);
});

const inventoryPlayersSearch = ref<string | null>("");

const inventorysPlayers = computed(() => {
  if (!claimFetch.value?.inventorys?.players?.length) {
    return [];
  }

  if (!inventoryPlayersSearch.value && !rarityPlayers.value && !tierPlayers.value) {
    return claimFetch.value?.inventorys?.players;
  }

  return claimFetch.value?.inventorys?.players.filter(
    (inventory) =>
      (!rarityPlayers.value || inventory.item.rarity === rarityPlayers.value) &&
      (!tierPlayers.value || inventory.item.tier === tierPlayers.value) &&
      (!inventoryPlayersSearch.value ||
        inventory.item.name.toLowerCase().includes(inventoryPlayersSearch.value.toLowerCase())),
  );
});

watch([rarityPlayers, tierPlayers], () => {
  playerItemsPage.value = 1;
});

const pagedInventoryPlayers = computed(() => {
  const start = (playerItemsPage.value - 1) * inventoryPageSize;
  return inventorysPlayers.value.slice(start, start + inventoryPageSize);
});

const inventoryPlayersOfflineSearch = ref<string | null>("");

const inventorysPlayersOffline = computed(() => {
  if (!claimFetch.value?.inventorys?.players_offline?.length) {
    return [];
  }

  if (
    !inventoryPlayersOfflineSearch.value &&
    !rarityPlayersOffline.value &&
    !tierPlayersOffline.value
  ) {
    return claimFetch.value?.inventorys?.players_offline;
  }

  return claimFetch.value?.inventorys?.players_offline.filter(
    (inventory) =>
      (!rarityPlayersOffline.value || inventory.item.rarity === rarityPlayersOffline.value) &&
      (!tierPlayersOffline.value || inventory.item.tier === tierPlayersOffline.value) &&
      (!inventoryPlayersOfflineSearch.value ||
        inventory.item.name
          .toLowerCase()
          .includes(inventoryPlayersOfflineSearch.value.toLowerCase())),
  );
});

watch([rarityPlayersOffline, tierPlayersOffline], () => {
  playerOfflineItemsPage.value = 1;
});

const pagedInventoryPlayersOffline = computed(() => {
  const start = (playerOfflineItemsPage.value - 1) * inventoryPageSize;
  return inventorysPlayersOffline.value.slice(start, start + inventoryPageSize);
});

const inventoryPlayersToolsSearch = ref<string | null>("");

const inventorysPlayersTools = computed(() => {
  if (!claimFetch.value?.tool_inventorys?.players?.length) {
    return [];
  }

  if (!inventoryPlayersToolsSearch.value && !rarityPlayersTools.value && !tierPlayersTools.value) {
    return claimFetch.value?.tool_inventorys?.players.slice().sort(sortToolInventory);
  }

  return claimFetch.value?.tool_inventorys?.players
    .filter(
      (inventory) =>
        (!rarityPlayersTools.value || inventory.item.rarity === rarityPlayersTools.value) &&
        (!tierPlayersTools.value || inventory.item.tier === tierPlayersTools.value) &&
        (!inventoryPlayersToolsSearch.value ||
          inventory.item.name
            .toLowerCase()
            .includes(inventoryPlayersToolsSearch.value.toLowerCase())),
    )
    .sort(sortToolInventory);
});

watch([rarityPlayersTools, tierPlayersTools], () => {
  playerToolsPage.value = 1;
});

const pagedInventoryPlayersTools = computed(() => {
  const start = (playerToolsPage.value - 1) * inventoryPageSize;
  return inventorysPlayersTools.value.slice(start, start + inventoryPageSize);
});

const inventoryPlayersOfflineToolsSearch = ref<string | null>("");

const inventorysPlayersOfflineTools = computed(() => {
  if (!claimFetch.value?.tool_inventorys?.players_offline?.length) {
    return [];
  }

  if (
    !inventoryPlayersOfflineToolsSearch.value &&
    !rarityPlayersOfflineTools.value &&
    !tierPlayersOfflineTools.value
  ) {
    return claimFetch.value?.tool_inventorys?.players_offline.slice().sort(sortToolInventory);
  }

  return claimFetch.value?.tool_inventorys?.players_offline
    .filter(
      (inventory) =>
        (!rarityPlayersOfflineTools.value ||
          inventory.item.rarity === rarityPlayersOfflineTools.value) &&
        (!tierPlayersOfflineTools.value || inventory.item.tier === tierPlayersOfflineTools.value) &&
        (!inventoryPlayersOfflineToolsSearch.value ||
          inventory.item.name
            .toLowerCase()
            .includes(inventoryPlayersOfflineToolsSearch.value.toLowerCase())),
    )
    .sort(sortToolInventory);
});

watch([rarityPlayersOfflineTools, tierPlayersOfflineTools], () => {
  playerOfflineToolsPage.value = 1;
});

const pagedInventoryPlayersOfflineTools = computed(() => {
  const start = (playerOfflineToolsPage.value - 1) * inventoryPageSize;
  return inventorysPlayersOfflineTools.value.slice(start, start + inventoryPageSize);
});

const claimOwner = computed(() => {
  if (claim.value === undefined) {
    return "";
  }

  return claim.value.members[claim.value.owner_player_entity_id];
});

useSeoMeta({
  title: () => `Claim ${claimFetch.value?.name ?? route.params.id}`,
  description: () =>
    `Claim ${claimFetch.value?.name ?? route.params.id} by ${claimOwner.value} with members (${claim.value?.members?.length || 0})  buildings (${buildings.value.length}) and items (${inventorysBuildings.value.length}) `,
});

const secondsToDaysMinutesSecondsFormat = (seconds: number) => {
  const years = Math.floor(seconds / (60 * 60 * 24 * 365));
  const weeks = Math.floor((seconds % (60 * 60 * 24 * 365)) / (60 * 60 * 24 * 7));
  const days = Math.floor((seconds % (60 * 60 * 24 * 7)) / (60 * 60 * 24));
  const hours = Math.floor((seconds % (60 * 60 * 24)) / (60 * 60));
  const minutes = Math.floor((seconds % (60 * 60)) / 60);
  const secondsLeft = seconds % 60;

  let result = "";

  if (years > 0) {
    result += `${years}y `;
  }

  if (weeks > 0) {
    result += `${weeks}w `;
  }

  if (days > 0) {
    result += `${days}d `;
  }

  if (hours > 0) {
    result += `${hours}h `;
  }

  if (minutes > 0) {
    result += `${minutes}m `;
  }

  if (secondsLeft > 0) {
    result += `${secondsLeft}s`;
  }

  return result;
};

const validTabs = new Set([
  "members",
  "building_items",
  "player_items",
  "player_tools",
  "player_offline_items",
  "player_offline_tools",
  "buildings",
  "leaderboards",
  "upgrades",
  "inventory_changelogs",
  "traveler_tasks",
]);

const initialTab = (() => {
  const next = (route.query.tab as string) || "";
  if (next && validTabs.has(next)) {
    return next;
  }
  return "members";
})();

let tab = ref(initialTab);

let memberSearch = ref<string | null>(null);
let showOnlyOnlineMembers = ref(false);
const memberSorting = ref<SortingState>([]);

const defaultMemberSort = (rows: any[]) => {
  return rows.slice().sort((a, b) => {
    if (a.permissions !== b.permissions) {
      return b.permissions - a.permissions;
    }
    if (a.online_state !== b.online_state) {
      return a.online_state === "Online" ? -1 : 1;
    }
    return a.user_name.localeCompare(b.user_name);
  });
};

const membersForTable = computed(() => {
  if (!claim.value?.members) {
    return [];
  }
  const rows = Object.values(claim.value.members)
    .filter((member) => {
      if (showOnlyOnlineMembers.value && member.online_state !== "Online") {
        return false;
      }

      if (memberSearch.value) {
        return member.user_name.toLowerCase().includes(memberSearch.value.toLowerCase());
      }

      return true;
    })
    .map((member) => {
      let permissions = 0;
      if (claimOwner.id === member.entity_id) {
        permissions += 18;
      }

      if (member.co_owner_permission) {
        permissions += 14;
      }
      if (member.co_owner_permission) {
        permissions += 12;
      }
      if (member.officer_permission) {
        permissions += 10;
      }
      if (member.build_permission) {
        permissions += 8;
      }
      if (member.inventory_permission) {
        permissions += 6;
      }

      return {
        ...member,
        permissions,
      };
    });

  return defaultMemberSort(rows);
});

const onMemberSearch = () => {
  if (tab.value !== "members") {
    tab.value = "members";
  }
};

const onlinePlayersCount = computed(() => {
  if (claimFetch.value === undefined) {
    return 0;
  }
  return (
    Object.values(claimFetch.value?.members).filter((member) => member.online_state === "Online")
      .length ?? 0
  );
});

const memberSkills = [
  "Carpentry",
  "Farming",
  "Forestry",
  "Foraging",
  "Tailoring",
  "Fishing",
  "Hunting",
  "Leatherworking",
  "Mining",
  "Smithing",
  "Masonry",
  "Scholar",
] as const;

const memberSecondarySkills = [
  "Cooking",
  "Sailing",
  "Slayer",
  "Taming",
  "Construction",
  "Merchanting",
] as const;

const memberColumns = computed(() => {
  return [
    {
      id: "user",
      header: "User",
      accessorFn: (row: any) => row.user_name,
      enableSorting: true,
    },
    {
      id: "permissions",
      header: "Perms",
      accessorKey: "permissions",
      enableSorting: true,
      meta: { class: { th: "text-center", td: "text-center" } },
    },
    ...memberSkills.toSorted().map((skill) => ({
      id: `skill_${skill}`,
      header: skill,
      accessorFn: (row: any) => row?.skills_ranks?.[skill] ?? 0,
      enableSorting: true,
      meta: { class: { th: "text-center", td: "text-center" } },
    })),
    ...memberSecondarySkills.toSorted().map((skill) => ({
      id: `skill_${skill}`,
      header: skill,
      accessorFn: (row: any) => row?.skills_ranks?.[skill] ?? 0,
      enableSorting: true,
      meta: { class: { th: "text-center", td: "text-center" } },
    })),
  ];
});

const claimTier = computed(() => claimFetch.value?.tier ?? 1);

const upgradesSorted = computed<ClaimTechDesc[]>(() => {
  if (!claimFetch.value?.upgrades?.length) {
    return [];
  }

  return claimFetch.value.upgrades
    .filter((upgrade) => upgrade.tech_type !== "TierUpgrade")
    .sort((a, b) => {
      if (a.tier !== b.tier) {
        return a.tier - b.tier;
      }

      return a.description.localeCompare(b.description);
    });
});

const upgradesByTier = computed(() => {
  const tiers = new Map<number, ClaimTechDesc[]>();

  upgradesSorted.value.forEach((upgrade) => {
    const tier = upgrade.tier ?? 1;
    const list = tiers.get(tier) ?? [];
    list.push(upgrade);
    tiers.set(tier, list);
  });

  return Array.from(tiers.entries()).sort((a, b) => a[0] - b[0]);
});

const tabItems = computed(() => {
  return [
    {
      value: "members",
      label: `Members (${membersForTable.value.length || 0})`,
    },
    {
      value: "player_items",
      label: `Member items (${inventorysPlayers.value.length || 0})`,
    },
    {
      value: "player_offline_items",
      label: `Member Offline items (${inventorysPlayersOffline.value.length || 0})`,
    },
    {
      value: "player_tools",
      label: `Member tools (${inventorysPlayersTools.value.length || 0})`,
    },
    {
      value: "player_offline_tools",
      label: `Member Offline tools (${inventorysPlayersOfflineTools.value.length || 0})`,
    },
    {
      value: "building_items",
      label: `Building items (${inventorysBuildings.value.length || 0})`,
    },
    { value: "buildings", label: `Buildings (${buildings.value.length || 0})` },
    // { value: "leaderboards", label: "Leaderboards" },
    {
      value: "upgrades",
      label: `Upgrades (${upgradesSorted.value.length || 0})`,
    },
    {
      value: "inventory_changelogs",
      label: `Inventory changes (${InventoryChangelogFetch.value?.length || 0})`,
    },
    { value: "traveler_tasks", label: "Traveler tasks" },
  ];
});

const selectedLocationEntries = computed<InventoryLocationEntry[]>(() => {
  return selectedInventoryLocation.value?.locations ?? [];
});

const uniqueSortedNumbers = (list: number[]) => {
  return Array.from(new Set(list)).sort((a, b) => a - b);
};

const uniqueSortedStrings = (list: string[]) => {
  return Array.from(new Set(list)).sort((a, b) => a.localeCompare(b));
};

const uniqueSortedStringsRarity = (list: string[]) => {
  return Array.from(new Set(list)).sort(raritySort);
};

const buildingTierOptions = computed(() => {
  return uniqueSortedNumbers(
    claimFetch.value?.inventorys?.buildings?.map((inventory) => inventory.item.tier) ?? [],
  );
});

const buildingRarityOptions = computed(() => {
  return uniqueSortedStringsRarity(
    claimFetch.value?.inventorys?.buildings?.map((inventory) => inventory.item.rarity) ?? [],
  );
});

const playerTierOptions = computed(() => {
  return uniqueSortedNumbers(
    claimFetch.value?.inventorys?.players?.map((inventory) => inventory.item.tier) ?? [],
  );
});

const playerRarityOptions = computed(() => {
  return uniqueSortedStringsRarity(
    claimFetch.value?.inventorys?.players?.map((inventory) => inventory.item.rarity) ?? [],
  );
});

const playerOfflineTierOptions = computed(() => {
  return uniqueSortedNumbers(
    claimFetch.value?.inventorys?.players_offline?.map((inventory) => inventory.item.tier) ?? [],
  );
});

const playerOfflineRarityOptions = computed(() => {
  return uniqueSortedStringsRarity(
    claimFetch.value?.inventorys?.players_offline?.map((inventory) => inventory.item.rarity) ?? [],
  );
});

const playerToolTierOptions = computed(() => {
  return uniqueSortedNumbers(
    claimFetch.value?.tool_inventorys?.players?.map((inventory) => inventory.item.tier) ?? [],
  );
});

const playerToolRarityOptions = computed(() => {
  return uniqueSortedStringsRarity(
    claimFetch.value?.tool_inventorys?.players?.map((inventory) => inventory.item.rarity) ?? [],
  );
});

const playerOfflineToolTierOptions = computed(() => {
  return uniqueSortedNumbers(
    claimFetch.value?.tool_inventorys?.players_offline?.map((inventory) => inventory.item.tier) ??
      [],
  );
});

const playerOfflineToolRarityOptions = computed(() => {
  return uniqueSortedStringsRarity(
    claimFetch.value?.tool_inventorys?.players_offline?.map((inventory) => inventory.item.rarity) ??
      [],
  );
});

const getPlayerNameById = (playerId: bigint) => {
  const key = playerId.toString();
  const member =
    claimFetch.value?.members?.[key] ??
    (claimFetch.value?.members as Record<string, any>)?.[playerId as unknown as string];
  return member?.user_name ?? `#${key}`;
};

const tierToBgStyle = (tier: number) => {
  return { backgroundColor: `rgba(var(--tier-${tier}), 0.10)` };
};

const levelToTier = (level: number) => {
  if (1 <= level && level <= 19) return 1;
  if (20 <= level && level <= 29) return 2;
  if (30 <= level && level <= 39) return 3;
  if (40 <= level && level <= 49) return 4;
  if (50 <= level && level <= 59) return 5;
  if (60 <= level && level <= 69) return 6;
  if (70 <= level && level <= 79) return 7;
  if (80 <= level && level <= 89) return 8;
  if (90 <= level && level <= 99) return 9;
  if (100 <= level) return 10;
  return 1;
};

const getSkillTool = (member: any, skill: (typeof memberSkills)[number]) => {
  const index = skillToToolIndex[skill as keyof typeof skillToToolIndex];
  if (index === undefined || index === null) {
    return null;
  }
  return member?.inventory?.pockets?.[index]?.contents?.item ?? null;
};

const getToolLabel = (item: any) => {
  if (!item?.tier) {
    return null;
  }
  const rarity = item.rarity ? Array.from(item.rarity)[0] : "";
  return `T${item.tier} ${rarity}`.trim();
};

const isBankLocation = (location: InventoryLocationEntry) => {
  const buildingName = location.building_name ?? location.owner_name ?? "";
  return location.owner_type === "Building" && buildingName.includes("Bank");
};

const formatLocationOwner = (location: InventoryLocationEntry) => {
  const ownerId = location.owner_entity_id.toString();
  const fallbackName = location.owner_name ?? `#${ownerId}`;

  if (location.owner_type === "Player") {
    const playerName = location.owner_name ?? getPlayerNameById(location.owner_entity_id);
    return `Player: ${playerName}`;
  }

  if (location.owner_type === "Building") {
    const buildingName = location.building_name ?? fallbackName;
    return `Building: ${buildingName}`;
  }

  return `Unknown: ${fallbackName}`;
};

const locationOwnerLink = (location: InventoryLocationEntry) => {
  const ownerId = location.owner_entity_id.toString();

  if (location.owner_type === "Player") {
    return { name: "players-id", params: { id: ownerId } };
  }

  if (location.owner_type === "Building") {
    return { name: "buildings-id", params: { id: ownerId } };
  }

  return null;
};

const bankOwnerLink = (location: InventoryLocationEntry) => {
  const playerId = location.player_owner_entity_id.toString();
  return { name: "players-id", params: { id: playerId } };
};

const openItemLocationModal = (
  inventory: ExpendedRefrence,
  section: string,
  source: "inventory" | "tool" = "inventory",
) => {
  selectedInventoryItem.value = inventory;
  const locations =
    source === "tool"
      ? (claimFetch.value?.tool_inventory_locations?.[section] ?? [])
      : (claimFetch.value?.inventory_locations?.[section] ?? []);
  selectedInventoryLocation.value =
    locations.find(
      (entry) => entry.item_id === inventory.item_id && entry.item_type === inventory.item_type,
    ) ?? null;
  locationModalOpen.value = true;
};

const isUpgradeLocked = (upgrade: ClaimTechDesc) => {
  return claimTier.value < upgrade.tier;
};

const unlockedUpgradesCount = computed(() => {
  return upgradesSorted.value.filter((upgrade) => !isUpgradeLocked(upgrade)).length;
});

const learnedUpgradesSet = computed(() => {
  return new Set(claimFetch.value?.learned_upgrades ?? []);
});

const isUpgradeLearned = (upgrade: ClaimTechDesc) => {
  return learnedUpgradesSet.value.has(upgrade.id);
};

const upgradeNameById = computed(() => {
  const map = new Map<number, string>();
  upgradesSorted.value.forEach((upgrade) => {
    map.set(upgrade.id, upgrade.name);
  });
  return map;
});

const unlocksTechNames = (upgrade: ClaimTechDesc) => {
  if (!upgrade.unlocks_techs?.length) {
    return [];
  }

  return upgrade.unlocks_techs.map((id) => {
    return upgradeNameById.value.get(id) ?? `#${id}`;
  });
};

const travelerColumns = [
  { id: "items", header: "Items" },
  { id: "name", header: "Name" },
  { id: "npc_name", header: "NPC" },
  { id: "player_count", header: "Players" },
  { id: "users", header: "User names" },
];

const getTravelerItemIcon = (shownItem: { item_type: "Item" | "Cargo"; item_id: number }) => {
  if (!itemsAndCargoAllFetch.value) {
    return null;
  }
  const desc =
    shownItem.item_type === "Item"
      ? itemsAndCargoAllFetch.value?.item_desc?.[shownItem.item_id]
      : itemsAndCargoAllFetch.value?.cargo_desc?.[shownItem.item_id];
  if (!desc?.icon_asset_name) {
    return null;
  }
  const icon = iconAssetUrlNameRandom(desc.icon_asset_name);
  return icon.show ? icon.url : null;
};

const skillToToolIndex = {
  Carpentry: 1,
  Construction: 13,
  Cooking: 10,
  Experience: undefined,
  Farming: 8,
  Fishing: 9,
  Foraging: 11,
  Forestry: 0,
  Hunting: 6,
  Leatherworking: 5,
  Level: undefined,
  Masonry: 2,
  Mining: 3,
  Scholar: 12,
  Slayer: 15,
  Smithing: 4,
  Tailoring: 7,
};

const numberFormat = new Intl.NumberFormat(undefined);

watchThrottled(
  () => [item_object.value, player_id.value],
  (value, oldValue) => {
    InventoryChangelogRefresh();
  },
  { throttle: 50 },
);

watch(
  () => route.query.tab,
  (value) => {
    const next = typeof value === "string" ? value : "";
    if (next && validTabs.has(next) && tab.value !== next) {
      tab.value = next;
    }
  },
  { immediate: true },
);

watch(
  tab,
  (value) => {
    if (!value || !validTabs.has(value)) {
      return;
    }

    if (route.query.tab !== value) {
      router.replace({ query: { ...route.query, tab: value } });
    }
  },
  { immediate: false },
);

watch(
  inventoryBuildingsSearch,
  (value, oldValue) => {
    if (value !== oldValue) {
      buildingItemsPage.value = 1;
    }
  },
  { immediate: false },
);

watch(
  inventoryPlayersSearch,
  (value, oldValue) => {
    if (value !== oldValue) {
      playerItemsPage.value = 1;
    }
  },
  { immediate: false },
);

watch(
  inventoryPlayersOfflineSearch,
  (value, oldValue) => {
    if (value !== oldValue) {
      playerOfflineItemsPage.value = 1;
    }
  },
  { immediate: false },
);

watch(
  inventoryPlayersToolsSearch,
  (value, oldValue) => {
    if (value !== oldValue) {
      playerToolsPage.value = 1;
    }
  },
  { immediate: false },
);

watch(
  inventoryPlayersOfflineToolsSearch,
  (value, oldValue) => {
    if (value !== oldValue) {
      playerOfflineToolsPage.value = 1;
    }
  },
  { immediate: false },
);
</script>

<template>
  <UContainer class="w-full max-w-none py-6">
    <template v-if="!claim">
      <div class="flex items-center justify-center py-10">
        <UProgress color="neutral" />
      </div>
    </template>
    <template v-else>
      <div class="flex flex-col gap-4">
        <div
          class="rounded-2xl border border-gray-200 bg-white/70 p-6 shadow-sm dark:border-neutral-800 dark:bg-neutral-900/40"
        >
          <div class="flex flex-wrap items-start justify-between gap-3">
            <div>
              <p class="text-xs uppercase tracking-[0.2em] text-gray-500 dark:text-gray-400">
                Claim
              </p>
              <h1 class="text-2xl font-semibold tracking-tight" :class="tierToColor[claimTier]">
                {{ claim.name }}
              </h1>
              <div class="mt-1 text-sm text-gray-600 dark:text-gray-300">
                <span class="font-semibold text-gray-800 dark:text-gray-100">Owner:</span>
                {{ claimOwner?.user_name ?? "Unknown" }}
              </div>
              <div class="mt-1 text-sm text-gray-600 dark:text-gray-300">
                <span class="font-semibold text-gray-800 dark:text-gray-100">Members: </span>
                <span class="text-emerald-600 dark:text-emerald-400">{{ onlinePlayersCount }}</span
                >/{{ claimFetch ? Object.values(claimFetch.members).length : 0 }}
              </div>
              <div
                v-if="claim?.location && claim?.location.x !== 0 && claim?.location.z !== 0"
                class="mt-1 text-sm text-gray-600 dark:text-gray-300"
              >
                R: <bitcraft-region :region="claim.region" /> N:
                {{ Math.ceil(claim.location.z / 3) }}, E: {{ Math.ceil(claim.location.x / 3) }}
              </div>
            </div>
            <div class="flex flex-wrap gap-2">
              <UBadge color="neutral" variant="soft">
                Supplies: {{ numberFormat.format(claim.supplies ?? 0) }}
              </UBadge>
              <UBadge color="neutral" variant="soft">
                Tiles: {{ numberFormat.format(claim.num_tiles ?? 0) }}
              </UBadge>
              <UBadge color="neutral" variant="soft">
                Treasury: {{ numberFormat.format(claim.treasury ?? 0) }}
              </UBadge>
              <UBadge color="neutral" variant="soft">
                Buildings: {{ numberFormat.format(claimFetch?.building_states?.length || 0) }}
              </UBadge>
              <UBadge color="neutral" variant="soft">
                Time signed in:
                {{ secondsToDaysMinutesSecondsFormat(claimFetch?.time_signed_in ?? 0) || "0s" }}
              </UBadge>
              <UBadge color="neutral" variant="soft">Tier {{ claimTier }}</UBadge>
            </div>
          </div>
        </div>

        <UCard :ui="{ body: 'sm:p-1 p-0', header: 'sm:p-3 p-0' }">
          <template #header>
            <div class="flex flex-wrap items-center">
              <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100">Claim details</h2>
            </div>
          </template>
          <div>
            <div class="flex flex-wrap gap-2 p-4">
              <UButton
                v-for="item in tabItems"
                :key="item.value"
                :label="item.label"
                :variant="tab === item.value ? 'solid' : 'soft'"
                size="xs"
                @click="tab = item.value"
              />
            </div>
            <div class="border-t border-gray-200 p-4 dark:border-gray-800">
              <div v-if="tab === 'members'" class="flex flex-col gap-3">
                <div class="flex flex-wrap items-center justify-between gap-3">
                  <div class="flex items-center gap-2">
                    <UBadge :color="onlinePlayersCount > 0 ? 'success' : 'neutral'" variant="soft">
                      {{ onlinePlayersCount }} online
                    </UBadge>
                    <span class="text-sm text-gray-500 dark:text-gray-400">
                      {{ claimFetch ? Object.values(claimFetch.members).length : 0 }} members
                    </span>
                  </div>
                  <div class="flex flex-wrap items-center gap-2">
                    <UInput
                      v-model="memberSearch"
                      icon="i-heroicons-magnifying-glass"
                      placeholder="Search members"
                      class="w-full sm:w-64"
                      @update:model-value="onMemberSearch"
                    />
                    <USwitch v-model="showOnlyOnlineMembers" label="Only online" />
                  </div>
                </div>
                <UTable
                  v-model:sorting="memberSorting"
                  :columns="memberColumns"
                  :data="membersForTable"
                  :sorting-options="{ getSortedRowModel: getSortedRowModel() }"
                  class="claim-table"
                >
                  <template #user-header="{ column }">
                    <UButton
                      variant="ghost"
                      size="xs"
                      class="-ml-2 font-semibold uppercase tracking-[0.08em]"
                      @click="column.toggleSorting(column.getIsSorted() === 'asc')"
                    >
                      User
                      <span class="ml-1 text-xs">
                        {{
                          column.getIsSorted() === "asc"
                            ? "▲"
                            : column.getIsSorted() === "desc"
                              ? "▼"
                              : ""
                        }}
                      </span>
                    </UButton>
                  </template>
                  <template #permissions-header="{ column }">
                    <UButton
                      variant="ghost"
                      size="xs"
                      class="-ml-2 font-semibold uppercase tracking-[0.08em]"
                      @click="column.toggleSorting(column.getIsSorted() === 'asc')"
                    >
                      Perms
                      <span class="ml-1 text-xs">
                        {{
                          column.getIsSorted() === "asc"
                            ? "▲"
                            : column.getIsSorted() === "desc"
                              ? "▼"
                              : ""
                        }}
                      </span>
                    </UButton>
                  </template>
                  <template
                    v-for="skill in memberSkills"
                    :key="`${skill}-header`"
                    #[`skill_${skill}-header`]="{ column }"
                  >
                    <UButton
                      variant="ghost"
                      size="xs"
                      class="-ml-2 font-semibold uppercase tracking-[0.08em]"
                      @click="column.toggleSorting(column.getIsSorted() === 'asc')"
                    >
                      {{ skill }}
                      <span class="ml-1 text-xs">
                        {{
                          column.getIsSorted() === "asc"
                            ? "▲"
                            : column.getIsSorted() === "desc"
                              ? "▼"
                              : ""
                        }}
                      </span>
                    </UButton>
                  </template>
                  <template
                    v-for="skill in memberSecondarySkills"
                    :key="`${skill}-header`"
                    #[`skill_${skill}-header`]="{ column }"
                  >
                    <UButton
                      variant="ghost"
                      size="xs"
                      class="-ml-2 font-semibold uppercase tracking-[0.08em]"
                      @click="column.toggleSorting(column.getIsSorted() === 'asc')"
                    >
                      {{ skill }}
                      <span class="ml-1 text-xs">
                        {{
                          column.getIsSorted() === "asc"
                            ? "▲"
                            : column.getIsSorted() === "desc"
                              ? "▼"
                              : ""
                        }}
                      </span>
                    </UButton>
                  </template>
                  <template #user-cell="{ row }">
                    <NuxtLink
                      :to="{ name: 'players-id', params: { id: row.original.entity_id } }"
                      class="font-semibold hover:underline"
                      :class="
                        row.original.online_state === 'Online'
                          ? 'text-emerald-600 dark:text-emerald-400'
                          : 'text-gray-900 dark:text-gray-100'
                      "
                    >
                      {{ row.original.user_name }}
                    </NuxtLink>
                    <div class="text-xs text-gray-500 dark:text-gray-400">
                      {{ row.original.online_state }}
                    </div>
                  </template>
                  <template #permissions-cell="{ row }">
                    <div class="flex items-center justify-center gap-1 text-lg">
                      <span v-if="row.original.co_owner_permission">🏰</span>
                      <span v-if="row.original.officer_permission">🗡️</span>
                      <span v-if="row.original.build_permission">🔨</span>
                      <span v-if="row.original.inventory_permission">📦</span>
                    </div>
                  </template>
                  <template
                    v-for="skill in memberSkills"
                    :key="skill"
                    #[`skill_${skill}-cell`]="{ row }"
                  >
                    <div class="flex items-center justify-center">
                      <span
                        class="rounded-l-full border-r-0 px-2 py-1 text-sm font-bold"
                        :class="levelToColor(row.original?.skills_ranks?.[skill] ?? 0)"
                        :style="
                          tierToBgStyle(levelToTier(row.original?.skills_ranks?.[skill] ?? 0))
                        "
                      >
                        {{ row.original?.skills_ranks?.[skill] ?? 0 }}
                      </span>
                      <span
                        v-if="getToolLabel(getSkillTool(row.original, skill))"
                        class="rounded-r-full px-2 py-1 text-sm font-bold"
                        :class="tierToColor[getSkillTool(row.original, skill)?.tier]"
                        :style="tierToBgStyle(getSkillTool(row.original, skill)?.tier || 1)"
                      >
                        {{ getToolLabel(getSkillTool(row.original, skill)) }}
                      </span>
                      <span
                        v-else
                        class="rounded-r-full px-2 py-1 text-sm text-gray-400 dark:text-gray-500"
                      >
                        --
                      </span>
                    </div>
                  </template>
                  <template
                    v-for="skill in memberSecondarySkills"
                    :key="skill"
                    #[`skill_${skill}-cell`]="{ row }"
                  >
                    <div class="flex items-center justify-center">
                      <span
                        class="rounded-l-full border-r-0 px-2 py-1 text-sm font-bold"
                        :class="`${levelToColor(row.original?.skills_ranks?.[skill] ?? 0)} ${skillToToolIndex[skill] ? '' : 'rounded-r-full'}`"
                        :style="
                          tierToBgStyle(levelToTier(row.original?.skills_ranks?.[skill] ?? 0))
                        "
                      >
                        {{ row.original?.skills_ranks?.[skill] ?? 0 }}
                      </span>
                      <span
                        v-if="getToolLabel(getSkillTool(row.original, skill))"
                        class="rounded-r-full px-2 py-1 text-sm font-bold"
                        :class="`${tierToColor[getSkillTool(row.original, skill)?.tier]}`"
                        :style="tierToBgStyle(getSkillTool(row.original, skill)?.tier || 1)"
                      >
                        {{ getToolLabel(getSkillTool(row.original, skill)) }}
                      </span>
                      <span
                        v-else-if="skillToToolIndex[skill] !== undefined"
                        class="rounded-r-full px-2 py-1 text-sm text-gray-400 dark:text-gray-500"
                      >
                        --
                      </span>
                    </div>
                  </template>
                </UTable>
              </div>

              <div v-else-if="tab === 'building_items'" class="flex flex-col gap-4">
                <div class="flex flex-wrap gap-2">
                  <UInput
                    v-model="inventoryBuildingsSearch"
                    icon="i-heroicons-magnifying-glass"
                    placeholder="Search items"
                    class="w-full sm:w-64"
                  />
                  <USelect
                    v-model="tierBuildings"
                    :items="buildingTierOptions"
                    placeholder="Tier"
                    class="w-32"
                  />
                  <USelectMenu
                    v-model="rarityBuildings"
                    :items="buildingRarityOptions"
                    placeholder="Rarity"
                    clear
                    class="w-40"
                  />
                </div>
                <div
                  class="grid gap-2 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 2xl:grid-cols-6"
                >
                  <UCard
                    v-for="inventory in pagedInventoryBuildings"
                    :key="`${inventory.item_id}-${inventory.item_type}`"
                    class="inventory-card border-l-4"
                    :class="tierToBorderClassByLevel(inventory.item.tier)"
                    role="button"
                    tabindex="0"
                    :ui="{ header: 'p-3', body: 'hidden' }"
                    @click="openItemLocationModal(inventory, 'buildings')"
                    @keydown.enter="openItemLocationModal(inventory, 'buildings')"
                    @keydown.space.prevent="openItemLocationModal(inventory, 'buildings')"
                  >
                    <template #header>
                      <div class="inventory-card__header inventory-card__header--media">
                        <div class="inventory-card__meta">
                          <InventoryImg :item="inventory.item" :width="48" :height="48" />
                          <div class="inventory-card__text">
                            <div
                              class="inventory-card__title"
                              :class="tierToColor[inventory.item.tier]"
                            >
                              {{ inventory.item.name }}
                            </div>
                            <div
                              class="inventory-card__subtitle"
                              :class="rarityToTextClass(inventory.item.rarity)"
                            >
                              {{ inventory.item.rarity }}
                            </div>
                          </div>
                        </div>
                        <div class="inventory-card__qty">
                          {{ numberFormat.format(inventory.quantity) }}
                        </div>
                      </div>
                    </template>
                  </UCard>
                </div>
                <div class="flex justify-center">
                  <UPagination
                    v-model:page="buildingItemsPage"
                    :total="inventorysBuildings.length"
                    :items-per-page="inventoryPageSize"
                  />
                </div>
              </div>

              <div v-else-if="tab === 'player_items'" class="flex flex-col gap-4">
                <div class="flex flex-wrap gap-2">
                  <UInput
                    v-model="inventoryPlayersSearch"
                    icon="i-heroicons-magnifying-glass"
                    placeholder="Search items"
                    class="w-full sm:w-64"
                  />
                  <USelect
                    v-model="tierPlayers"
                    :items="playerTierOptions"
                    placeholder="Tier"
                    class="w-32"
                  />
                  <USelectMenu
                    v-model="rarityPlayers"
                    :items="playerRarityOptions"
                    clear
                    placeholder="Rarity"
                    class="w-40"
                  />
                </div>
                <div
                  class="grid gap-2 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 2xl:grid-cols-6"
                >
                  <UCard
                    v-for="inventory in pagedInventoryPlayers"
                    :key="`${inventory.item_id}-${inventory.item_type}`"
                    class="inventory-card border-l-4"
                    :class="tierToBorderClassByLevel(inventory.item.tier)"
                    role="button"
                    tabindex="0"
                    :ui="{ header: 'p-3', body: 'hidden' }"
                    @click="openItemLocationModal(inventory, 'players')"
                    @keydown.enter="openItemLocationModal(inventory, 'players')"
                    @keydown.space.prevent="openItemLocationModal(inventory, 'players')"
                  >
                    <template #header>
                      <div class="inventory-card__header inventory-card__header--media">
                        <div class="inventory-card__meta">
                          <InventoryImg :item="inventory.item" :width="48" :height="48" />
                          <div class="inventory-card__text">
                            <div
                              class="inventory-card__title"
                              :class="tierToColor[inventory.item.tier]"
                            >
                              {{ inventory.item.name }}
                            </div>
                            <div
                              class="inventory-card__subtitle"
                              :class="rarityToTextClass(inventory.item.rarity)"
                            >
                              {{ inventory.item.rarity }}
                            </div>
                          </div>
                        </div>
                        <div class="inventory-card__qty">
                          {{ numberFormat.format(inventory.quantity) }}
                        </div>
                      </div>
                    </template>
                  </UCard>
                </div>
                <div class="flex justify-center">
                  <UPagination
                    v-model:page="playerItemsPage"
                    :total="inventorysPlayers.length"
                    :items-per-page="inventoryPageSize"
                  />
                </div>
              </div>

              <div v-else-if="tab === 'player_tools'" class="flex flex-col gap-4">
                <div class="flex flex-wrap gap-2">
                  <UInput
                    v-model="inventoryPlayersToolsSearch"
                    icon="i-heroicons-magnifying-glass"
                    placeholder="Search tools"
                    class="w-full sm:w-64"
                  />
                  <USelect
                    v-model="tierPlayersTools"
                    :items="playerToolTierOptions"
                    placeholder="Tier"
                    class="w-32"
                  />
                  <USelectMenu
                    v-model="rarityPlayersTools"
                    :items="playerToolRarityOptions"
                    clear
                    placeholder="Rarity"
                    class="w-40"
                  />
                  <span class="align-middle"
                    >Total Items:
                    {{
                      pagedInventoryPlayersTools.reduce(
                        (accumulator, currentValue) => accumulator + currentValue.quantity,
                        0,
                      )
                    }}</span
                  >
                </div>
                <div
                  class="grid gap-2 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 2xl:grid-cols-6"
                >
                  <UCard
                    v-for="inventory in pagedInventoryPlayersTools"
                    :key="`${inventory.item_id}-${inventory.item_type}`"
                    class="inventory-card border-l-4"
                    :class="tierToBorderClassByLevel(inventory.item.tier)"
                    role="button"
                    tabindex="0"
                    :ui="{ header: 'p-3', body: 'hidden' }"
                    @click="openItemLocationModal(inventory, 'players', 'tool')"
                    @keydown.enter="openItemLocationModal(inventory, 'players', 'tool')"
                    @keydown.space.prevent="openItemLocationModal(inventory, 'players', 'tool')"
                  >
                    <template #header>
                      <div class="inventory-card__header inventory-card__header--media">
                        <div class="inventory-card__meta">
                          <InventoryImg :item="inventory.item" :width="48" :height="48" />
                          <div class="inventory-card__text">
                            <div
                              class="inventory-card__title"
                              :class="tierToColor[inventory.item.tier]"
                            >
                              {{ inventory.item.name }}
                            </div>
                            <div
                              class="inventory-card__subtitle"
                              :class="rarityToTextClass(inventory.item.rarity)"
                            >
                              {{ inventory.item.rarity }}
                            </div>
                          </div>
                        </div>
                        <div class="inventory-card__qty">
                          {{ numberFormat.format(inventory.quantity) }}
                        </div>
                      </div>
                    </template>
                  </UCard>
                </div>
                <div class="flex justify-center">
                  <UPagination
                    v-model:page="playerToolsPage"
                    :total="inventorysPlayersTools.length"
                    :items-per-page="inventoryPageSize"
                  />
                </div>
              </div>

              <div v-else-if="tab === 'player_offline_items'" class="flex flex-col gap-4">
                <div class="flex flex-wrap gap-2">
                  <UInput
                    v-model="inventoryPlayersOfflineSearch"
                    icon="i-heroicons-magnifying-glass"
                    placeholder="Search items"
                    class="w-full sm:w-64"
                  />
                  <USelect
                    v-model="tierPlayersOffline"
                    :items="playerOfflineTierOptions"
                    placeholder="Tier"
                    class="w-32"
                  />
                  <USelectMenu
                    v-model="rarityPlayersOffline"
                    :items="playerOfflineRarityOptions"
                    placeholder="Rarity"
                    clear
                    class="w-40"
                  />
                </div>
                <div
                  class="grid gap-2 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 2xl:grid-cols-6"
                >
                  <UCard
                    v-for="inventory in pagedInventoryPlayersOffline"
                    :key="`${inventory.item_id}-${inventory.item_type}`"
                    class="inventory-card border-l-4"
                    :class="tierToBorderClassByLevel(inventory.item.tier)"
                    role="button"
                    tabindex="0"
                    :ui="{ header: 'p-3', body: 'hidden' }"
                    @click="openItemLocationModal(inventory, 'players_offline')"
                    @keydown.enter="openItemLocationModal(inventory, 'players_offline')"
                    @keydown.space.prevent="openItemLocationModal(inventory, 'players_offline')"
                  >
                    <template #header>
                      <div class="inventory-card__header inventory-card__header--media">
                        <div class="inventory-card__meta">
                          <InventoryImg :item="inventory.item" :width="48" :height="48" />
                          <div class="inventory-card__text">
                            <div
                              class="inventory-card__title"
                              :class="tierToColor[inventory.item.tier]"
                            >
                              {{ inventory.item.name }}
                            </div>
                            <div
                              class="inventory-card__subtitle"
                              :class="rarityToTextClass(inventory.item.rarity)"
                            >
                              {{ inventory.item.rarity }}
                            </div>
                          </div>
                        </div>
                        <div class="inventory-card__qty">
                          {{ numberFormat.format(inventory.quantity) }}
                        </div>
                      </div>
                    </template>
                  </UCard>
                </div>
                <div class="flex justify-center">
                  <UPagination
                    v-model:page="playerOfflineItemsPage"
                    :total="inventorysPlayersOffline.length"
                    :items-per-page="inventoryPageSize"
                  />
                </div>
              </div>

              <div v-else-if="tab === 'player_offline_tools'" class="flex flex-col gap-4">
                <div class="flex flex-wrap gap-2">
                  <UInput
                    v-model="inventoryPlayersOfflineToolsSearch"
                    icon="i-heroicons-magnifying-glass"
                    placeholder="Search tools"
                    class="w-full sm:w-64"
                  />
                  <USelect
                    v-model="tierPlayersOfflineTools"
                    :items="playerOfflineToolTierOptions"
                    placeholder="Tier"
                    class="w-32"
                  />
                  <USelectMenu
                    v-model="rarityPlayersOfflineTools"
                    :items="playerOfflineToolRarityOptions"
                    clear
                    placeholder="Rarity"
                    class="w-40"
                  />
                  <span class="align-middle"
                    >Total Items:
                    {{
                      pagedInventoryPlayersOfflineTools.reduce(
                        (accumulator, currentValue) => accumulator + currentValue.quantity,
                        0,
                      )
                    }}</span
                  >
                </div>
                <div
                  class="grid gap-2 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 2xl:grid-cols-6"
                >
                  <UCard
                    v-for="inventory in pagedInventoryPlayersOfflineTools"
                    :key="`${inventory.item_id}-${inventory.item_type}`"
                    class="inventory-card border-l-4"
                    :class="tierToBorderClassByLevel(inventory.item.tier)"
                    role="button"
                    tabindex="0"
                    :ui="{ header: 'p-3', body: 'hidden' }"
                    @click="openItemLocationModal(inventory, 'players_offline', 'tool')"
                    @keydown.enter="openItemLocationModal(inventory, 'players_offline', 'tool')"
                    @keydown.space.prevent="
                      openItemLocationModal(inventory, 'players_offline', 'tool')
                    "
                  >
                    <template #header>
                      <div class="inventory-card__header inventory-card__header--media">
                        <div class="inventory-card__meta">
                          <InventoryImg :item="inventory.item" :width="48" :height="48" />
                          <div class="inventory-card__text">
                            <div
                              class="inventory-card__title"
                              :class="tierToColor[inventory.item.tier]"
                            >
                              {{ inventory.item.name }}
                            </div>
                            <div
                              class="inventory-card__subtitle"
                              :class="rarityToTextClass(inventory.item.rarity)"
                            >
                              {{ inventory.item.rarity }}
                            </div>
                          </div>
                        </div>
                        <div class="inventory-card__qty">
                          {{ numberFormat.format(inventory.quantity) }}
                        </div>
                      </div>
                    </template>
                  </UCard>
                </div>
                <div class="flex justify-center">
                  <UPagination
                    v-model:page="playerOfflineToolsPage"
                    :total="inventorysPlayersOfflineTools.length"
                    :items-per-page="inventoryPageSize"
                  />
                </div>
              </div>

              <div v-else-if="tab === 'buildings'" class="flex flex-col gap-3">
                <div class="flex flex-wrap items-center gap-2">
                  <UInput
                    v-model="search"
                    icon="i-heroicons-magnifying-glass"
                    placeholder="Search buildings"
                    class="w-full sm:w-64"
                  />
                </div>
                <UProgress v-if="buildingsPending" color="neutral" />
                <div class="grid gap-2 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
                  <NuxtLink
                    v-for="building in buildings"
                    :key="building.entity_id"
                    :to="{ name: 'buildings-id', params: { id: building.entity_id.toString() } }"
                    class="flex items-center gap-3 rounded-lg border border-gray-200 p-3 text-sm font-semibold text-gray-900 transition hover:border-gray-300 hover:bg-gray-50 dark:border-gray-800 dark:text-gray-100 dark:hover:bg-gray-900"
                  >
                    <img
                      v-if="iconDomain"
                      :src="`${iconDomain}/${getBuildingIcon(building.building_description_id)}.webp`"
                      alt=""
                      class="h-10 w-10 rounded-md object-cover"
                    />
                    <span>{{ building.building_name }}</span>
                  </NuxtLink>
                </div>
                <div class="flex justify-center">
                  <UPagination
                    v-model:page="page"
                    :total="buidlingsFetch?.total ?? 0"
                    :items-per-page="perPage"
                  />
                </div>
              </div>

              <div v-else-if="tab === 'leaderboards'">
                <LeaderboardClaim :claim-id="claim?.entity_id" />
              </div>

              <div v-else-if="tab === 'upgrades'" class="flex flex-col gap-4">
                <div class="grid gap-3 md:grid-cols-3">
                  <div class="rounded-lg border border-gray-200 p-3 dark:border-gray-800">
                    <div
                      class="text-xs uppercase tracking-[0.2em] text-gray-500 dark:text-gray-400"
                    >
                      Claim tier
                    </div>
                    <div class="text-lg font-semibold text-gray-900 dark:text-gray-100">
                      T{{ claimTier }}
                    </div>
                  </div>
                  <div class="rounded-lg border border-gray-200 p-3 dark:border-gray-800">
                    <div
                      class="text-xs uppercase tracking-[0.2em] text-gray-500 dark:text-gray-400"
                    >
                      Unlocked upgrades
                    </div>
                    <div class="text-lg font-semibold text-gray-900 dark:text-gray-100">
                      {{ unlockedUpgradesCount }} / {{ upgradesSorted.length || 0 }}
                    </div>
                  </div>
                </div>
                <div v-for="[tier, upgrades] in upgradesByTier" :key="tier" class="space-y-3">
                  <div class="flex flex-wrap items-center justify-between gap-2">
                    <h3 class="text-base font-semibold text-gray-900 dark:text-gray-100">
                      Tier {{ tier }}
                    </h3>
                    <UBadge variant="soft" color="neutral">{{ upgrades.length }} upgrades</UBadge>
                  </div>
                  <div class="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
                    <div
                      v-for="upgrade in upgrades"
                      :key="upgrade.id"
                      class="rounded-lg border border-gray-200 p-4 shadow-sm transition dark:border-gray-800"
                      :class="isUpgradeLocked(upgrade) ? 'opacity-70' : ''"
                    >
                      <div class="flex items-start justify-between gap-2">
                        <div>
                          <div class="text-sm font-semibold text-gray-900 dark:text-gray-100">
                            {{ upgrade.name }}
                          </div>
                          <div class="text-xs text-gray-500 dark:text-gray-400">
                            {{ upgrade.description }}
                          </div>
                        </div>
                        <UBadge variant="soft" color="neutral">T{{ upgrade.tier }}</UBadge>
                      </div>
                      <div class="mt-2 text-xs text-gray-500 dark:text-gray-400">
                        Type: {{ upgrade.tech_type }}
                      </div>
                      <div
                        v-if="upgrade.unlocks_techs?.length"
                        class="mt-1 text-xs text-gray-500 dark:text-gray-400"
                      >
                        Unlocks: {{ unlocksTechNames(upgrade).join(", ") }}
                      </div>
                      <div class="mt-3">
                        <UBadge v-if="isUpgradeLocked(upgrade)" variant="soft" color="neutral">
                          Requires Claim Tier {{ upgrade.tier }}
                        </UBadge>
                        <UBadge
                          v-else-if="isUpgradeLearned(upgrade)"
                          variant="soft"
                          color="primary"
                        >
                          Learned
                        </UBadge>
                        <UBadge v-else variant="soft" color="success">Available</UBadge>
                      </div>
                    </div>
                  </div>
                </div>
              </div>

              <div v-else-if="tab === 'inventory_changelogs'" class="flex flex-col gap-4">
                <div class="flex flex-wrap gap-2">
                  <AutocompleteUser @model_changed="(item) => (player_id = item)" />
                  <AutocompleteItem @model_changed="(item) => (item_object = item)" />
                </div>
                <InventoryChanges :items="InventoryChangelogFetch" />
              </div>

              <div v-else-if="tab === 'traveler_tasks'" class="flex flex-col gap-4">
                <UTable :columns="travelerColumns" :data="travelerTaskRows" class="claim-table">
                  <template #items-cell="{ row }">
                    <div class="flex flex-wrap gap-2">
                      <div
                        v-for="shownItem of row.original.items"
                        :key="`${shownItem.item_type}-${shownItem.item_id}`"
                        class="flex items-center gap-2"
                      >
                        <div class="relative">
                          <img
                            v-if="getTravelerItemIcon(shownItem)"
                            :src="getTravelerItemIcon(shownItem) || ''"
                            alt=""
                            class="h-10 w-10 rounded-md object-contain"
                          />
                          <span
                            class="absolute -right-2 -top-2 rounded-full bg-gray-900 px-1 text-[10px] font-semibold text-white dark:bg-gray-100 dark:text-gray-900"
                          >
                            {{ numberFormat.format(shownItem.quantity) }}
                          </span>
                        </div>
                      </div>
                    </div>
                  </template>
                  <template #name-cell="{ row }">
                    <div class="space-y-1">
                      <div
                        v-for="shownItem of row.original.items"
                        :key="`${shownItem.item_type}-${shownItem.item_id}-name`"
                        :class="
                          tierToColor[
                            shownItem.item_type === 'Item'
                              ? itemsAndCargoAllFetch?.item_desc?.[shownItem.item_id]?.tier
                              : itemsAndCargoAllFetch?.cargo_desc?.[shownItem.item_id]?.tier
                          ]
                        "
                      >
                        {{
                          shownItem.item_type === "Item"
                            ? itemsAndCargoAllFetch?.item_desc?.[shownItem.item_id]?.name
                            : itemsAndCargoAllFetch?.cargo_desc?.[shownItem.item_id]?.name
                        }}
                      </div>
                    </div>
                  </template>
                  <template #npc_name-cell="{ row }">
                    {{ row.original.npc_name }}
                  </template>
                  <template #player_count-cell="{ row }">
                    {{ row.original.player_count }}
                  </template>
                  <template #users-cell="{ row }">
                    <div class="flex flex-wrap gap-2">
                      <NuxtLink
                        v-for="playerId of row.original.players"
                        :key="playerId"
                        :to="{ name: 'players-id', params: { id: playerId } }"
                        class="text-sm font-semibold text-gray-900 hover:underline dark:text-gray-100"
                      >
                        {{ claimFetch.members[playerId]?.user_name }}
                      </NuxtLink>
                    </div>
                  </template>
                </UTable>
              </div>
            </div>
          </div>
        </UCard>
      </div>
    </template>

    <UModal v-model:open="locationModalOpen">
      <template #content>
        <UCard :ui="{ body: 'p-4' }">
          <template #header>
            <div class="flex items-center justify-between">
              <span class="text-lg font-semibold">Item location</span>
              <UButton
                icon="i-heroicons-x-mark"
                variant="ghost"
                @click="locationModalOpen = false"
              />
            </div>
          </template>
          <div class="flex items-center gap-3">
            <InventoryImg
              v-if="selectedInventoryItem"
              width="48"
              height="48"
              skip-error-text
              :item="selectedInventoryItem.item"
            />
            <div>
              <div class="text-sm font-semibold text-gray-900 dark:text-gray-100">
                {{ selectedInventoryItem?.item.name }}
              </div>
              <div class="text-xs text-gray-500 dark:text-gray-400">
                Quantity: {{ selectedInventoryItem?.quantity }}
              </div>
            </div>
          </div>
          <div class="mt-4 border-t border-gray-200 pt-3 text-sm dark:border-gray-800">
            <div
              v-if="selectedLocationEntries.length === 0"
              class="text-gray-500 dark:text-gray-400"
            >
              No location data available for this item.
            </div>
            <div v-else class="space-y-3">
              <div
                v-for="location in selectedLocationEntries"
                :key="`${location.owner_entity_id}-${location.inventory_index}-${location.cargo_index}`"
                class="rounded-lg border border-gray-200 p-3 text-sm dark:border-gray-800"
              >
                <div class="font-semibold text-gray-900 dark:text-gray-100">
                  <template v-if="locationOwnerLink(location)">
                    <NuxtLink :to="locationOwnerLink(location)" class="hover:underline">
                      {{ formatLocationOwner(location) }}
                    </NuxtLink>
                  </template>
                  <template v-else>
                    {{ formatLocationOwner(location) }}
                  </template>
                </div>
                <div class="mt-1 text-xs text-gray-500 dark:text-gray-400">
                  <template v-if="isBankLocation(location)">
                    <NuxtLink :to="bankOwnerLink(location)" class="hover:underline">
                      Bank owner: {{ getPlayerNameById(location.player_owner_entity_id) }}
                    </NuxtLink>
                    <span class="mx-1">•</span>
                  </template>
                  Quantity: {{ location.quantity }} | Inventory: {{ location.inventory_index }} |
                  Slot:
                  {{ location.cargo_index }}
                </div>
              </div>
            </div>
          </div>
        </UCard>
      </template>
    </UModal>
  </UContainer>
</template>

<style scoped>
.inventory-card {
  display: grid;
  text-align: left;
  transition:
    transform 0.2s ease,
    box-shadow 0.2s ease,
    border-color 0.2s ease;
  min-height: 0;
  cursor: pointer;
}

.inventory-card:hover {
  transform: translateY(-2px);
  box-shadow: 0 12px 24px -18px rgba(15, 23, 42, 0.4);
}

.inventory-card__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 8px;
}

.inventory-card__header--media {
  min-height: 48px;
}

.inventory-card__meta {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
}

.inventory-card__text {
  min-width: 0;
}

.inventory-card__title {
  text-transform: uppercase;
  font-size: 1rem;
  line-height: 1.1;
}

.inventory-card__subtitle {
  font-size: 0.75rem;
}

.inventory-card__qty {
  font-size: 1rem;
  white-space: nowrap;
}

.claim-table :deep(thead tr th) {
  font-size: 0.75rem;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: rgba(100, 116, 139, 0.9);
}

.claim-table :deep(tbody tr td) {
  vertical-align: top;
}
</style>
