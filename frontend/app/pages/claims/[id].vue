<script setup lang="ts">
import type { SortingState } from "@tanstack/vue-table";
import { watchThrottled } from "@vueuse/shared";

const toast = useToast();

import ClaimTabBuildings from "~/components/Bitcraft/Claim/ClaimTabBuildings.vue";
import ClaimTabInventoryChangelogs from "~/components/Bitcraft/Claim/ClaimTabInventoryChangelogs.vue";
import ClaimTabLeaderboards from "~/components/Bitcraft/Claim/ClaimTabLeaderboards.vue";
import ClaimTabMembers from "~/components/Bitcraft/Claim/ClaimTabMembers.vue";
import ClaimTabTravelerTasks from "~/components/Bitcraft/Claim/ClaimTabTravelerTasks.vue";
import ClaimTabUpgrades from "~/components/Bitcraft/Claim/ClaimTabUpgrades.vue";
import InventoryImg from "~/components/Bitcraft/InventoryImg.vue";
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
import { raritySort } from "~/utils";
import type { BuildingDescriptionsResponse } from "~/types/BuildingDescriptionsResponse";
import type { ClaimDescriptionStateMember } from "~/types/ClaimDescriptionStateMember";
import type { ItemExpended } from "~/types/ItemExpended";
import ClaimTabInventorySection from "~/components/Bitcraft/Claim/ClaimTabInventorySection.vue";

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

const sortToolInventory = (a: ExpendedRefrence, b: ExpendedRefrence) => {
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

const { data: claimFetch } = useFetchMsPack<ClaimDescriptionStateWithInventoryAndPlayTime>(() => {
  return `/api/bitcraft/claims/${route.params.id.toString()}`;
});

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
        triggerRef(claimFetch);
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
    triggerRef(claimFetch);
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
    triggerRef(claimFetch);
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

const defaultMemberSort = (rows: ({ permissions: number } & ClaimDescriptionStateMember)[]) => {
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
  const rows = Object.values<{ permissions: number } & ClaimDescriptionStateMember>(
    claim.value.members,
  )
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
      accessorFn: (row) => row.user_name,
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
      accessorFn: (row) => row?.skills_ranks?.[skill] ?? 0,
      enableSorting: true,
      meta: { class: { th: "text-center", td: "text-center" } },
    })),
    ...memberSecondarySkills.toSorted().map((skill) => ({
      id: `skill_${skill}`,
      header: skill,
      accessorFn: (row) => row?.skills_ranks?.[skill] ?? 0,
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
    { value: "leaderboards", label: "Leaderboards" },
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
  const member = claimFetch.value?.members?.[key] ?? claimFetch.value?.members?.[playerId];
  return member?.user_name ?? `#${key}`;
};

const tierToBgStyle = (tier: number) => {
  return { backgroundColor: `rgba(var(--tier-${tier}), 0.10)` };
};

const getSkillTool = (
  member: ClaimDescriptionStateMember,
  skill: (typeof memberSkills)[number],
) => {
  const index = skillToToolIndex[skill as keyof typeof skillToToolIndex];
  if (index === undefined || index === null) {
    return undefined;
  }
  return member?.inventory?.pockets?.[index]?.contents?.item;
};

const getToolLabel = (item: ItemExpended) => {
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
              <h1 class="text-2xl font-semibold tracking-tight" :class="tierToColor[claimTier]">
                {{ claim.name }}
              </h1>
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
                <span class="text-gray-800 dark:text-gray-100">Owner:</span>
                {{ claimOwner?.user_name ?? "Unknown" }}
              </UBadge>
              <UBadge color="neutral" variant="soft">
                <span class="text-gray-800 dark:text-gray-100">Members: </span>
                <span class="text-emerald-600 dark:text-emerald-400">{{ onlinePlayersCount }}</span
                >/<span class="text-gray-800 dark:text-gray-100">{{
                  claimFetch ? Object.values(claimFetch.members).length : 0
                }}</span>
              </UBadge>
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
              <ClaimTabMembers
                v-if="tab === 'members'"
                :online-players-count="onlinePlayersCount"
                :member-count="claimFetch ? Object.values(claimFetch.members).length : 0"
                :member-search="memberSearch"
                :show-only-online-members="showOnlyOnlineMembers"
                :member-sorting="memberSorting"
                :member-columns="memberColumns"
                :members-for-table="membersForTable"
                :member-skills="memberSkills"
                :member-secondary-skills="memberSecondarySkills"
                :tier-to-color="tierToColor"
                :skill-to-tool-index="skillToToolIndex"
                :tier-to-bg-style="tierToBgStyle"
                :get-skill-tool="getSkillTool"
                :get-tool-label="getToolLabel"
                @update:member-search="(value) => (memberSearch = value)"
                @update:show-only-online-members="(value) => (showOnlyOnlineMembers = value)"
                @update:member-sorting="(value) => (memberSorting = value)"
                @member-search-changed="onMemberSearch"
              />

              <ClaimTabInventorySection
                v-else-if="tab === 'building_items'"
                :search="inventoryBuildingsSearch"
                :tier="tierBuildings"
                :rarity="rarityBuildings"
                :tier-options="buildingTierOptions"
                :rarity-options="buildingRarityOptions"
                :items="pagedInventoryBuildings"
                :page="buildingItemsPage"
                :total="inventorysBuildings.length"
                :items-per-page="inventoryPageSize"
                search-placeholder="Search items"
                location-section="buildings"
                :tier-to-color="tierToColor"
                :number-format="numberFormat"
                :open-item-location-modal="openItemLocationModal"
                @update:search="(value) => (inventoryBuildingsSearch = value)"
                @update:tier="(value) => (tierBuildings = value)"
                @update:rarity="(value) => (rarityBuildings = value)"
                @update:page="(value) => (buildingItemsPage = value)"
              />

              <ClaimTabInventorySection
                v-else-if="tab === 'player_items'"
                :search="inventoryPlayersSearch"
                :tier="tierPlayers"
                :rarity="rarityPlayers"
                :tier-options="playerTierOptions"
                :rarity-options="playerRarityOptions"
                :items="pagedInventoryPlayers"
                :page="playerItemsPage"
                :total="inventorysPlayers.length"
                :items-per-page="inventoryPageSize"
                search-placeholder="Search items"
                location-section="players"
                :tier-to-color="tierToColor"
                :number-format="numberFormat"
                :open-item-location-modal="openItemLocationModal"
                @update:search="(value) => (inventoryPlayersSearch = value)"
                @update:tier="(value) => (tierPlayers = value)"
                @update:rarity="(value) => (rarityPlayers = value)"
                @update:page="(value) => (playerItemsPage = value)"
              />

              <ClaimTabInventorySection
                v-else-if="tab === 'player_tools'"
                :search="inventoryPlayersToolsSearch"
                :tier="tierPlayersTools"
                :rarity="rarityPlayersTools"
                :tier-options="playerToolTierOptions"
                :rarity-options="playerToolRarityOptions"
                :items="pagedInventoryPlayersTools"
                :page="playerToolsPage"
                :total="inventorysPlayersTools.length"
                :items-per-page="inventoryPageSize"
                search-placeholder="Search tools"
                location-section="players"
                tool-mode
                show-total-items
                :tier-to-color="tierToColor"
                :number-format="numberFormat"
                :open-item-location-modal="openItemLocationModal"
                @update:search="(value) => (inventoryPlayersToolsSearch = value)"
                @update:tier="(value) => (tierPlayersTools = value)"
                @update:rarity="(value) => (rarityPlayersTools = value)"
                @update:page="(value) => (playerToolsPage = value)"
              />

              <ClaimTabInventorySection
                v-else-if="tab === 'player_offline_items'"
                :search="inventoryPlayersOfflineSearch"
                :tier="tierPlayersOffline"
                :rarity="rarityPlayersOffline"
                :tier-options="playerOfflineTierOptions"
                :rarity-options="playerOfflineRarityOptions"
                :items="pagedInventoryPlayersOffline"
                :page="playerOfflineItemsPage"
                :total="inventorysPlayersOffline.length"
                :items-per-page="inventoryPageSize"
                search-placeholder="Search items"
                location-section="players_offline"
                :tier-to-color="tierToColor"
                :number-format="numberFormat"
                :open-item-location-modal="openItemLocationModal"
                @update:search="(value) => (inventoryPlayersOfflineSearch = value)"
                @update:tier="(value) => (tierPlayersOffline = value)"
                @update:rarity="(value) => (rarityPlayersOffline = value)"
                @update:page="(value) => (playerOfflineItemsPage = value)"
              />

              <ClaimTabInventorySection
                v-else-if="tab === 'player_offline_tools'"
                :search="inventoryPlayersOfflineToolsSearch"
                :tier="tierPlayersOfflineTools"
                :rarity="rarityPlayersOfflineTools"
                :tier-options="playerOfflineToolTierOptions"
                :rarity-options="playerOfflineToolRarityOptions"
                :items="pagedInventoryPlayersOfflineTools"
                :page="playerOfflineToolsPage"
                :total="inventorysPlayersOfflineTools.length"
                :items-per-page="inventoryPageSize"
                search-placeholder="Search tools"
                location-section="players_offline"
                tool-mode
                show-total-items
                :tier-to-color="tierToColor"
                :number-format="numberFormat"
                :open-item-location-modal="openItemLocationModal"
                @update:search="(value) => (inventoryPlayersOfflineToolsSearch = value)"
                @update:tier="(value) => (tierPlayersOfflineTools = value)"
                @update:rarity="(value) => (rarityPlayersOfflineTools = value)"
                @update:page="(value) => (playerOfflineToolsPage = value)"
              />

              <ClaimTabBuildings
                v-else-if="tab === 'buildings'"
                :search="search"
                :buildings="buildings"
                :buildings-pending="buildingsPending"
                :page="page"
                :total="Number(buidlingsFetch?.total ?? 0)"
                :per-page="perPage"
                :icon-domain="iconDomain"
                :get-building-icon="getBuildingIcon"
                @update:search="(value) => (search = value)"
                @update:page="(value) => (page = value)"
              />

              <ClaimTabLeaderboards
                v-else-if="tab === 'leaderboards'"
                :claim-id="claim?.entity_id"
              />

              <ClaimTabUpgrades
                v-else-if="tab === 'upgrades'"
                :claim-tier="claimTier"
                :unlocked-upgrades-count="unlockedUpgradesCount"
                :upgrades-sorted="upgradesSorted"
                :upgrades-by-tier="upgradesByTier"
                :is-upgrade-locked="isUpgradeLocked"
                :is-upgrade-learned="isUpgradeLearned"
                :unlocks-tech-names="unlocksTechNames"
              />

              <ClaimTabInventoryChangelogs
                v-else-if="tab === 'inventory_changelogs'"
                :items="InventoryChangelogFetch"
                @player-changed="(item) => (player_id = item)"
                @item-changed="(item) => (item_object = item)"
              />

              <ClaimTabTravelerTasks
                v-else-if="tab === 'traveler_tasks'"
                :traveler-columns="travelerColumns"
                :traveler-task-rows="travelerTaskRows"
                :claim-members="claimFetch.members"
                :items-and-cargo-all-fetch="itemsAndCargoAllFetch"
                :tier-to-color="tierToColor"
                :number-format="numberFormat"
                :get-traveler-item-icon="getTravelerItemIcon"
              />
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
