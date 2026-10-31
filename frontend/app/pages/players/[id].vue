<script setup lang="ts">
import Inventory from "~/components/Bitcraft/Inventory.vue";
import { iconAssetUrlNameRandom } from "~/composables/iconAssetName";
import { registerWebsocketMessageHandler } from "~/composables/websocket";
import type { FindPlayerByIdResponse } from "~/types/FindPlayerByIdResponse";
import type { HouseResponse } from "~/types/HouseResponse";
import type { HouseInventoriesResponse } from "~/types/HouseInventoriesResponse";
import type { InventorysResponse } from "~/types/InventorysResponse";
import type { ItemsAndCargollResponse } from "~/types/ItemsAndCargollResponse";
import type { NpcDesc } from "~/types/NpcDesc";
import type { PlayerLeaderboardResponse } from "~/types/PlayerLeaderboardResponse";
import type { RankType } from "~/types/RankType";
import type { TravelerTaskDesc } from "~/types/TravelerTaskDesc";
import { useDelayedPending } from "~/utils";
import HouseDetails from "~/components/Bitcraft/HouseDetails.vue";
import { inventoryMatchesSearch } from "~/utils/inventorySearch";

const toast = useToast();

const page = ref(1);
const route = useRoute();
const numberFormat = new Intl.NumberFormat(undefined);
const nDate = Intl.DateTimeFormat(undefined, {
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  hour12: false,
});

const tmpPage = (route.query.page as string) ?? null;

const topics = reactive<string[]>([`experience.${route.params.id}`]);
const topics_total_experience = reactive<string[]>([`total_experience.${route.params.id}`]);

const mobileEntityStateTopics = computed(() => {
  return [`mobile_entity_state.${route.params.id}`];
});

registerWebsocketMessageHandler("MobileEntityState", mobileEntityStateTopics, (message) => {
  if (playerData.value) {
    playerData.value.player_location = message;
  }
});

const playerActionStateTopics = computed(() => {
  return [`player_action_state_change_name.${route.params.id}`];
});

registerWebsocketMessageHandler(
  "PlayerActionStateChangeName",
  playerActionStateTopics,
  (message) => {
    if (playerData.value) {
      playerData.value.player_action_state = message[0];
    }
  },
);

registerWebsocketMessageHandler("Experience", topics, (message) => {
  if (experienceData.value && experienceData.value[message.skill_name]) {
    const currentLevel = experienceData.value[message.skill_name].level;

    experienceData.value[message.skill_name].experience = message.experience;
    experienceData.value[message.skill_name].level = message.level;

    if (currentLevel !== message.level && currentLevel <= message.level) {
      toast.add({
        title: `Level ${message.level} reached for Skill ${message.skill_name}`,
      });

      experienceData.value["Level"].level += 1;
    }
  }
});

registerWebsocketMessageHandler("TotalExperience", topics_total_experience, (message) => {
  if (experienceData.value && experienceData.value["Experience"]) {
    experienceData.value["Experience"].experience = message.experience;
    experienceData.value["Experience"].rank = message.rank;
  }
});

const topics_profession_experience = reactive<string[]>([
  `total_profession_experience.${route.params.id}`,
]);
registerWebsocketMessageHandler(
  "TotalProfessionExperience",
  topics_profession_experience,
  (message) => {
    if (experienceData.value && experienceData.value["Professions"]) {
      experienceData.value["Professions"].experience = message.experience;
      experienceData.value["Professions"].rank = message.rank;
    }
  },
);

const topics_adventure_experience = reactive<string[]>([
  `total_adventure_experience.${route.params.id}`,
]);
registerWebsocketMessageHandler(
  "TotalAdventureExperience",
  topics_adventure_experience,
  (message) => {
    if (experienceData.value && experienceData.value["Skills"]) {
      experienceData.value["Skills"].experience = message.experience;
      experienceData.value["Skills"].rank = message.rank;
    }
  },
);

const topicsPlayer = reactive<string[]>([`player_state.${route.params.id}`]);

registerWebsocketMessageHandler("PlayerState", topicsPlayer, (message) => {
  if (playerData.value) {
    if (playerData.value.signed_in !== message.signed_in) {
      if (message.signed_in) {
        toast.add({
          title: `${playerData.value?.username} signed in`,
        });
      } else {
        toast.add({
          title: `${playerData.value?.username} signed out`,
        });
      }
    }

    playerData.value = {
      ...playerData.value,
      signed_in: message.signed_in,
      time_signed_in: message.time_signed_in,
      time_played: message.time_played,
    };
  }
});

if (tmpPage) {
  page.value = parseInt(tmpPage);
}

const { data: playerData, pending: playerPending } = useFetchMsPack<FindPlayerByIdResponse>(() => {
  return `/api/bitcraft/players/${route.params.id}`;
});

const { data: houses } = await useLazyFetchMsPack<HouseResponse[]>(
  () => `/api/bitcraft/houses/by_player/${route.params.id}`,
);

const { data: houseInventories } = useFetchMsPack<HouseInventoriesResponse[]>(
  () => `/api/bitcraft/houses/by_player/${route.params.id}/inventories`,
  { deep: true },
);

const { data: inventoryData } = useFetchMsPack<InventorysResponse>(
  () => {
    return `/api/bitcraft/inventorys/owner_entity_id/${route.params.id}`;
  },
  { deep: true },
);

const houseSelection = ref<Record<string, boolean>>({});
const houseSelectionKey = () => `b-tool@house-storage:${route.params.id}`;

const loadHouseSelection = () => {
  houseSelection.value = {};
  if (import.meta.server) return;
  try {
    const stored = JSON.parse(localStorage.getItem(houseSelectionKey()) ?? "{}");
    if (stored && typeof stored === "object" && !Array.isArray(stored)) {
      houseSelection.value = Object.fromEntries(
        Object.entries(stored).filter(([, value]) => typeof value === "boolean"),
      ) as Record<string, boolean>;
    }
  } catch {
    // Keep the default selection if browser storage is unavailable or invalid.
  }
};

onMounted(loadHouseSelection);
watch(() => route.params.id, loadHouseSelection);

const storageHouses = computed(() => {
  const storageIds = new Set((houseInventories.value ?? []).map((house) => String(house.house_entity_id)));
  return (houses.value ?? [])
    .filter((house) => storageIds.has(String(house.entity_id)))
    .sort((a, b) =>
      Number(String(b.owner_entity_id) === String(route.params.id)) -
      Number(String(a.owner_entity_id) === String(route.params.id)),
    );
});

const isHouseSelected = (house: HouseResponse) =>
  houseSelection.value[String(house.entity_id)] ??
  String(house.owner_entity_id) === String(route.params.id);

const setHouseSelected = (house: HouseResponse, selected: boolean | "indeterminate") => {
  houseSelection.value[String(house.entity_id)] = selected === true;
  try {
    localStorage.setItem(houseSelectionKey(), JSON.stringify(houseSelection.value));
  } catch {
    // The selection still works for this visit if storage is unavailable.
  }
};

const inventorySources = () => [
  ...(inventoryData.value?.inventorys ?? []),
  ...(houseInventories.value ?? []).flatMap((house) => house.inventories),
];

const allInventories = computed(() => {
  const storageByInventory = new Map<string, string>();
  for (const storage of houseInventories.value ?? []) {
    for (const inventory of storage.inventories) {
      storageByInventory.set(String(inventory.entity_id), String(storage.house_entity_id));
    }
  }
  const selected = new Set(storageHouses.value.filter(isHouseSelected).map((house) => String(house.entity_id)));
  return Array.from(new Map(inventorySources()
    .filter((inventory) => {
      const houseId = storageByInventory.get(String(inventory.entity_id));
      return !houseId || selected.has(houseId);
    })
    .map((inventory) => [String(inventory.entity_id), inventory])).values());
});

const houseByInventory = computed(() => {
  const result = new Map<string, HouseResponse>();
  for (const storage of houseInventories.value ?? []) {
    const house = houses.value?.find((entry) => String(entry.entity_id) === String(storage.house_entity_id));
    if (house) {
      for (const inventory of storage.inventories) result.set(String(inventory.entity_id), house);
    }
  }
  return result;
});

const inventoryUpdateTopics = computed(() => {
  return [...new Set(inventorySources().map((inventory) => `inventory_update.${inventory.entity_id}`))];
});

registerWebsocketMessageHandler("InventoryUpdate", inventoryUpdateTopics, (message) => {
  for (const inventory of inventorySources()) {
    if (String(inventory.entity_id) === String(message.resolved_inventory.entity_id)) {
      inventory.pockets = message.resolved_inventory.pockets;
      inventory.region = message.resolved_inventory.region;
    }
  }
});

const inventoryRemoveTopics = computed(() => {
  return [...new Set(inventorySources().map((inventory) => `inventory_remove.${inventory.entity_id}`))];
});

registerWebsocketMessageHandler("InventoryRemove", inventoryRemoveTopics, (message) => {
  for (const list of [inventoryData.value?.inventorys, ...(houseInventories.value ?? []).map((house) => house.inventories)]) {
    if (!list) continue;
    const index = list.findIndex((value) => String(message.resolved_inventory.entity_id) === String(value.entity_id));
    if (index !== -1) list.splice(index, 1);
  }
});

registerWebsocketMessageHandler(
  "InventoryInsert",
  [`inventory_insert_player_owner.${route.params.id}`],
  (message) => {
    const index = inventoryData.value.inventorys.findIndex(
      (value) => message.resolved_inventory.entity_id === value.entity_id,
    );

    if (index !== -1) {
      inventoryData.value.inventorys[index].pockets = message.resolved_inventory.pockets;
      inventoryData.value.inventorys[index].region = message.resolved_inventory.region;
    } else {
      inventoryData.value.inventorys.push(message.resolved_inventory);
    }
  },
);

const { data: npcData } = useFetchMsPack<Record<number, NpcDesc>>(() => {
  return `/npc`;
});
const { data: trevelerTasksData } = useFetchMsPack<{
  [key: number]: TravelerTaskDesc;
}>(() => {
  return `/traveler_tasks`;
});

const { data: itemsAndCargoAllData } = useFetchMsPack<ItemsAndCargollResponse>(() => {
  return `/api/bitcraft/itemsAndCargo/all`;
});

const { data: experienceData } = useFetchMsPack<PlayerLeaderboardResponse>(
  () => {
    return `/api/bitcraft/experience/${route.params.id}`;
  },
  { deep: true },
);

const expeirence = computed(() => {
  if (!experienceData.value) {
    return undefined;
  }

  const newExperience: Record<
    string,
    RankType & {
      classes: Record<string, string>;
    }
  > = {
    Experience: {},
    Professions: {},
    Skills: {},
    Level: {},
  };

  for (const [skill, xp_info] of Object.entries(experienceData.value)) {
    let shouldAddClass = true;

    if (
      skill === "Experience" ||
      skill === "Level" ||
      skill === "Skills" ||
      skill === "Professions"
    ) {
      shouldAddClass = false;
    }

    newExperience[skill] = {
      experience: xp_info.experience,
      level: xp_info.level,
      rank: xp_info.rank,
      classes: {
        list: shouldAddClass ? `background-tier-${levelToTier(xp_info.level)}` : "",
        container: shouldAddClass ? "container" : "",
        content: shouldAddClass ? "content" : "",
      },
    };
  }

  return newExperience;
});

const inventorySearch = ref("");
const inventorySearchQuery = computed(() => inventorySearch.value.trim());

const mainInventory = computed(() =>
  inventoryData.value?.inventorys.find(
    (inventory) =>
      String(inventory.owner_entity_id) === String(route.params.id) &&
      inventory.inventory_index === 0,
  ),
);

const orderedInventories = computed(() =>
  [...allInventories.value].sort(
    (a, b) => Number(String(b.entity_id) === String(mainInventory.value?.entity_id)) - Number(String(a.entity_id) === String(mainInventory.value?.entity_id)),
  ),
);

const inventoryList = computed(() => {
  return orderedInventories.value.filter((inventory) => {
    const nickname = inventory.nickname;
    const isWalletOrTool = nickname === "Wallet" || nickname === "Tool belt";
    const hasContents = !!inventory.pockets.find((pocket) => pocket.contents?.quantity);

    return !isWalletOrTool && hasContents;
  });
});

const filteredInventoryList = computed(() => {
  if (!inventorySearchQuery.value) return inventoryList.value;
  return orderedInventories.value.filter((inventory) =>
    inventoryMatchesSearch(inventory, inventorySearchQuery.value),
  );
});

const tools = computed(() => {
  return (
    inventoryData.value?.inventorys.find((inventory) => inventory.nickname === "Tool belt") ??
    undefined
  );
});

const wallet = computed(() => {
  return (
    inventoryData.value?.inventorys.find((inventory) => inventory.nickname === "Wallet") ??
    undefined
  );
});

const deployables = computed(() => {
  return [...(playerData.value?.deployables ?? [])].sort((a, b) =>
    a.collectible_desc.tag.localeCompare(b.collectible_desc.tag) ||
    a.collectible_desc.name.localeCompare(b.collectible_desc.name, undefined, { numeric: true }),
  );
});

const secondsToDaysMinutesSecondsFormat = (seconds: number) => {
  const days = Math.floor(seconds / (60 * 60 * 24));
  const hours = Math.floor((seconds % (60 * 60 * 24)) / (60 * 60));
  const minutes = Math.floor((seconds % (60 * 60)) / 60);
  const secondsLeft = seconds % 60;

  let result = "";

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

const getRankName = (rank: number) => {
  switch (rank) {
    case 7:
      return "Owner";
    case 6:
      return "Admin";
    case 5:
      return "Resident";
    case 1:
      return "Guest";
    default:
      return `Rank ${rank}`;
  }
};

useSeoMeta({
  title: () => `Player ${playerData.value?.username ?? route.params.id}`,
  description: () => `Player ${playerData.value?.username ?? route.params.id}`,
});

const showPlayerPending = useDelayedPending(playerPending, 150);

const claims = computed(() => playerData.value?.claims ?? []);

const travelerTaskGroups = computed(() => {
  if (!playerData.value?.traveler_tasks || !npcData.value || !trevelerTasksData.value) {
    return [];
  }

  return Object.entries(playerData.value.traveler_tasks).map(([npcIndex, tasks]) => {
    const index = Number(npcIndex);
    const npc = npcData.value?.[index];
    const items = tasks.flatMap((task) =>
      (trevelerTasksData.value?.[task.task_id]?.required_items || []).map((item) => ({
        task,
        item,
      })),
    );

    return {
      npcName: npc?.name ?? `Traveler ${index + 1}`,
      items,
      completedCount: tasks.filter((task) => task.completed).length,
      totalCount: tasks.length,
    };
  });
});

const showTravelerTasks = ref(false);

const travelerTaskSummary = computed(() => {
  const completed = travelerTaskGroups.value.reduce(
    (total, group) => total + group.completedCount,
    0,
  );
  const total = travelerTaskGroups.value.reduce((sum, group) => sum + group.totalCount, 0);

  return {
    completed,
    total,
  };
});

const loginAt = computed(() => {
  const timestamp = playerData.value?.sign_in_timestamp;
  if (!timestamp) return null;
  const date = new Date(timestamp * 1000);
  if (date.getFullYear() === 1970) return null;
  return nDate.format(date);
});

const formatQuantity = (value: number | bigint | null | undefined) => {
  if (value === null || value === undefined) return "0";
  return typeof value === "bigint"
    ? numberFormat.format(Number(value))
    : numberFormat.format(value);
};
</script>

<template>
  <UContainer class="w-full max-w-none py-4">
    <template v-if="showPlayerPending">
      <div class="flex items-center justify-center py-8">
        <UProgress color="neutral" />
      </div>
    </template>
    <template v-else-if="playerData">
      <div class="flex flex-col gap-3">
        <div
          class="flex flex-col gap-2 rounded-2xl border border-gray-200 bg-white/70 p-6 shadow-sm dark:border-gray-800 dark:bg-gray-900/40"
        >
          <div class="flex flex-wrap items-center justify-between gap-2">
            <div>
              <p class="text-xs uppercase tracking-[0.2em] text-gray-500 dark:text-gray-400">
                Player
              </p>
              <h1
                class="text-2xl font-semibold tracking-tight"
                :class="
                  playerData.signed_in ? 'text-green-600' : 'text-gray-900 dark:text-gray-100'
                "
              >
                {{ playerData.username }}
              </h1>
              <div
                v-if="playerData.player_location"
                class="text-sm text-gray-600 dark:text-gray-300"
              >
                N: {{ Math.floor(playerData.player_location.location_z / 3 / 1000) }} E:
                {{ Math.floor(playerData.player_location.location_x / 3 / 1000) }}
                <span class="mx-1">•</span>
                <bitcraft-region
                  v-if="playerData.player_location.region"
                  :region="playerData.player_location.region"
                />
              </div>
            </div>
            <div class="flex flex-wrap items-center gap-2">
              <UBadge :color="playerData.signed_in ? 'success' : 'neutral'" variant="soft">
                {{ playerData.signed_in ? "Online" : "Offline" }}
              </UBadge>
              <UBadge color="warning" variant="soft">
                Hex Coins: {{ formatQuantity(wallet?.pockets[0]?.contents?.quantity) }}
              </UBadge>
            </div>
          </div>
          <div class="flex flex-wrap gap-2 text-xs text-gray-500 dark:text-gray-400">
            <div class="rounded-full border border-gray-200 px-3 py-1 dark:border-gray-800">
              Played: {{ secondsToDaysMinutesSecondsFormat(playerData.time_played) || "0s" }}
            </div>
            <div class="rounded-full border border-gray-200 px-3 py-1 dark:border-gray-800">
              Signed in: {{ secondsToDaysMinutesSecondsFormat(playerData.time_signed_in) || "0s" }}
            </div>
            <div
              v-if="loginAt"
              class="rounded-full border border-gray-200 px-3 py-1 dark:border-gray-800"
            >
              Login at: {{ loginAt }}
            </div>
          </div>
        </div>

        <UCard :ui="{ body: 'p-4' }">
          <div class="flex flex-col gap-3">
            <div class="flex flex-wrap items-center justify-between gap-2">
              <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100">Overview</h2>
              <UBadge v-if="playerData.player_action_state" color="info" variant="soft">
                {{ playerData.player_action_state }}
              </UBadge>
            </div>
            <div v-if="claims.length" class="flex flex-wrap gap-2">
              <NuxtLink
                v-for="claim in claims"
                :key="claim.entity_id.toString()"
                :to="{ name: 'claims-id', params: { id: claim.entity_id.toString() } }"
              >
                <UBadge color="neutral" variant="soft"> Claim: {{ claim.name }} </UBadge>
              </NuxtLink>
            </div>
            <UEmpty
              v-else
              icon="i-lucide-flag"
              title="No claims"
              description="This player does not own any claims yet."
            />
          </div>
        </UCard>

        <UCard v-if="deployables && deployables.length" :ui="{ body: 'p-4' }">
          <div class="flex flex-col gap-2">
            <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100">Deployables</h2>
            <div class="flex flex-col gap-2 sm:flex-row sm:flex-wrap sm:items-start">
              <div
                v-for="deployable in deployables"
                :key="deployable.id"
                class="flex max-w-full items-center gap-3 rounded-lg border border-gray-200 p-3 dark:border-gray-800 sm:w-fit"
              >
                <bitcraft-inventory-img
                  :item="deployable.collectible_desc"
                  :width="48"
                  :height="48"
                  class="size-12 shrink-0"
                />
                <div class="min-w-0">
                  <p
                    class="text-sm font-semibold"
                    :class="deployable.activated ? 'text-green-600 dark:text-green-400' : 'text-gray-900 dark:text-gray-100'"
                  >
                    {{ deployable.collectible_desc.name }}
                  </p>
                  <p class="text-xs text-gray-500 dark:text-gray-400">
                    Amount: {{ numberFormat.format(deployable.count) }}
                  </p>
                </div>
              </div>
            </div>
          </div>
        </UCard>

        <UCard v-if="expeirence" :ui="{ body: 'p-4' }">
          <div class="flex flex-col gap-2">
            <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100">Skills</h2>
            <div class="grid gap-2 lg:grid-cols-3 xl:grid-cols-4">
              <bitcraft-player-skill
                v-for="[skill, xp_info] of Object.entries(expeirence)"
                :key="skill"
                :xp_info="xp_info"
                :skill="skill"
                :tools="tools"
              />
            </div>
          </div>
        </UCard>

        <UCard v-if="travelerTaskGroups.length" :ui="{ body: 'p-4' }">
          <div class="flex flex-col gap-3">
            <div class="flex flex-wrap items-center justify-between gap-2">
              <div>
                <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100">
                  Traveler tasks
                </h2>
                <p class="text-xs text-gray-500 dark:text-gray-400">
                  {{ travelerTaskSummary.completed }}/{{ travelerTaskSummary.total }} completed
                </p>
              </div>
              <USwitch v-model="showTravelerTasks" label="Show tasks" />
            </div>
            <div v-if="showTravelerTasks" class="grid gap-2 lg:grid-cols-2">
              <div
                v-for="group in travelerTaskGroups"
                :key="group.npcName"
                class="rounded-lg border border-gray-200 p-3 dark:border-gray-800"
              >
                <div class="flex flex-wrap items-center justify-between gap-2">
                  <p class="text-sm font-semibold text-gray-900 dark:text-gray-100">
                    {{ group.npcName }}
                  </p>
                  <UBadge color="neutral" variant="soft">
                    {{ group.completedCount }}/{{ group.totalCount }} completed
                  </UBadge>
                </div>
                <div class="mt-3 grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
                  <div
                    v-for="(entry, index) in group.items"
                    :key="`${group.npcName}-${index}`"
                    class="rounded-md border border-gray-200 p-2 text-xs dark:border-gray-800"
                  >
                    <p class="font-semibold text-gray-900 dark:text-gray-100">
                      {{
                        entry.item.item_type === "Item"
                          ? itemsAndCargoAllData?.item_desc?.[entry.item.item_id]?.name
                          : itemsAndCargoAllData?.cargo_desc?.[entry.item.item_id]?.name
                      }}
                    </p>
                    <p class="text-gray-500 dark:text-gray-400">
                      Qty: {{ numberFormat.format(entry.item.quantity) }}
                    </p>
                    <UBadge
                      class="mt-2"
                      :color="entry.task.completed ? 'success' : 'warning'"
                      variant="soft"
                    >
                      {{ entry.task.completed ? "Completed" : "Pending" }}
                    </UBadge>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </UCard>

        <UCard v-if="inventoryData?.inventorys.length || houseInventories?.length || houses?.length" :ui="{ body: 'p-4' }">
          <div class="flex flex-col gap-2">
            <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100">Inventories</h2>
            <UInput
              v-model="inventorySearch"
              icon="i-lucide-search"
              placeholder="Search all inventories for an item or cargo…"
              aria-label="Search all inventories for an item or cargo"
              class="w-full"
            >
              <template v-if="inventorySearch" #trailing>
                <UButton
                  icon="i-lucide-x"
                  color="neutral"
                  variant="link"
                  size="xs"
                  aria-label="Clear inventory search"
                  @click="inventorySearch = ''"
                />
              </template>
            </UInput>
            <p class="text-xs text-gray-500 dark:text-gray-400">
              Search items and cargo by name across player inventories and selected house storage.
            </p>
            <fieldset v-if="storageHouses.length" class="flex flex-wrap gap-x-5 gap-y-2 py-2">
              <legend class="mb-2 text-sm font-medium text-gray-700 dark:text-gray-200">
                Include house storage
              </legend>
              <UCheckbox
                v-for="house in storageHouses"
                :key="house.entity_id.toString()"
                :model-value="isHouseSelected(house)"
                @update:model-value="setHouseSelected(house, $event)"
              >
                <template #label>
                  {{ house.owner_username || "Unknown owner" }}
                  (Region <bitcraft-region :region="house.region_index" />)
                </template>
              </UCheckbox>
            </fieldset>
            <UEmpty
              v-if="inventorySearchQuery && !filteredInventoryList.length"
              icon="i-lucide-search"
              title="No matching items in inventories"
              description="Try another name."
            />
            <div class="inventory-masonry pt-2">
              <div
                v-for="inventory in filteredInventoryList"
                :key="inventory.entity_id"
                class="inventory-masonry-item"
              >
                <Inventory
                  :inventory="inventory"
                  :label="String(inventory.entity_id) === String(mainInventory?.entity_id) ? 'Player Inventory' : undefined"
                  :house="houseByInventory.get(String(inventory.entity_id))"
                  :search="inventorySearchQuery"
                  show-region
                />
              </div>
            </div>
          </div>
        </UCard>

        <UCard v-if="houses && houses.length" :ui="{ body: 'p-4' }">
          <div class="flex flex-col gap-2">
            <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100">
              Accessible houses ({{ houses.length }})
            </h2>
            <div class="flex flex-col gap-4 pt-2">
              <div
                v-for="house in houses"
                :key="house.entity_id.toString()"
                class="rounded-lg border border-gray-200 p-3 dark:border-gray-800"
              >
                <house-details :house="house" :search="inventorySearchQuery"></house-details>
              </div>
            </div>
          </div>
        </UCard>
      </div>
    </template>
    <template v-else>
      <UEmpty
        icon="i-lucide-user-x"
        title="Player not found"
        description="We couldn’t find a player with that ID."
      />
    </template>
  </UContainer>
</template>

<style scoped>
.inventory-masonry {
  column-count: 1;
  column-gap: 1rem;
}

.inventory-masonry-item {
  break-inside: avoid;
}

@media (min-width: 80rem) {
  .inventory-masonry:has(> .inventory-masonry-item:nth-child(2)) {
    column-count: 2;
  }
}
</style>
