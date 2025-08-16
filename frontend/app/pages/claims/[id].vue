<script setup lang="ts">
import LeaderboardClaim from "~/components/Bitcraft/LeaderboardClaim.vue";
import AutocompleteUser from "~/components/Bitcraft/autocomplete/AutocompleteUser.vue";
import AutocompleteItem from "~/components/Bitcraft/autocomplete/AutocompleteItem.vue";
import InventoryChanges from "~/components/Bitcraft/InventoryChanges.vue";
import { iconAssetUrlNameRandom } from "~/composables/iconAssetName";
import { watchThrottled } from "@vueuse/shared";
import { useNow } from "@vueuse/core";
import { registerWebsocketMessageHandler } from "~/composables/websocket";
import { toast } from "vuetify-sonner";
import type { ClaimDescriptionStateWithInventoryAndPlayTime } from "~/types/ClaimDescriptionStateWithInventoryAndPlayTime";
import type { BuildingStatesResponse } from "~/types/BuildingStatesResponse";
import type { InventoryChangelog } from "~/types/InventoryChangelog";
import type { ItemCargo } from "~/types/ItemCargo";
import type { TravelerTaskDesc } from "~/types/TravelerTaskDesc";
import type { ItemsAndCargollResponse } from "~/types/ItemsAndCargollResponse";

const {
  public: { iconDomain },
} = useRuntimeConfig();

const page = ref(1);
const buildingItemsPage = ref(1);
const perPage = 1500;

const building_items_collapsible = ref([]);
const player_items_collapsible = ref([]);
const player_offline_items_collapsible = ref([]);
const buildings_collapsible = ref([]);
const inventory_changelog_collapsible = ref([]);
const traveler_tasks_collapsible = ref([]);

const search = ref<string | null>("");

const route = useRoute();
const router = useRouter();

const player_id = ref<bigint | null>();
const item_object = ref<ItemCargo | undefined>();

const rarityBuildings = ref<string | null>(null);
const tierBuildings = ref<number | null>(null);

const rarityPlayers = ref<string | null>(null);
const tierPlayers = ref<number | null>(null);

const rarityPlayersOffline = ref<string | null>(null);
const tierPlayersOffline = ref<number | null>(null);

const tmpPage = (route.query.page as string) ?? null;

if (tmpPage) {
  page.value = parseInt(tmpPage);
}

const { data: claimFetch } =
  useFetchMsPack<ClaimDescriptionStateWithInventoryAndPlayTime>(
    () => {
      return `/api/bitcraft/claims/${route.params.id.toString()}`;
    },
    { deep: true },
  );

const { data: trevelerTasksFetch } = useFetchMsPack<{
  [key: number]: TravelerTaskDesc;
}>(() => {
  return `/traveler_tasks`;
});

const { data: itemsAndCargoAllFetch } = useFetchMsPack<ItemsAndCargollResponse>(
  () => {
    return `/api/bitcraft/itemsAndCargo/all`;
  },
);

const { data: InventoryChangelogFetch, refresh: InventoryChangelogRefresh } =
  useFetchMsPack<InventoryChangelog[]>(
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

const { data: buidlingsFetch, pending: buildingsPending } =
  useFetchMsPack<BuildingStatesResponse>(() => {
    return `/api/bitcraft/buildings?claim_entity_id=${route.params.id}&page=${page.value}&per_page=${perPage}&skip_static_buildings=true&with_inventory=true`;
  });

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
    if (message.entity_id == route.params.id) {
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

  if (
    claimFetch.value.members[message.entity_id].online_state !== onlineState
  ) {
    if (message.signed_in) {
      toast(
        `${claimFetch.value.members[message.entity_id].user_name} signed in`,
        {
          progressBar: true,
          duration: 5000,
        },
      );
    } else {
      toast(
        `${claimFetch.value.members[message.entity_id].user_name} signed out`,
        {
          progressBar: true,
          duration: 5000,
        },
      );
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
    toast(
      `Player ${claimFetch.value?.members[message.user_id].user_name} Level ${message.level} reached for Skill ${message.skill_name}`,
      { progressBar: true, duration: 5000 },
    );

    claimFetch.value.members[message.user_id].skills_ranks[
      message.skill_name
    ].level = message.level;
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
      !search.value ||
      building.building_name.toLowerCase().includes(search.value.toLowerCase())
    );
  });
});

const inventoryBuildingsSearch = ref<string | null>("");

const inventorysBuildings = computed(() => {
  if (!claimFetch.value?.inventorys?.buildings?.length) {
    return [];
  }

  if (
    !inventoryBuildingsSearch.value &&
    !rarityBuildings.value &&
    !tierBuildings.value
  ) {
    return claimFetch.value?.inventorys?.buildings;
  }

  return claimFetch.value?.inventorys?.buildings.filter(
    (inventory) =>
      (!rarityBuildings.value ||
        parseInt(Object.keys(inventory.item.rarity)[0]) ===
          rarityBuildings.value) &&
      (!tierBuildings.value || inventory.item.tier === tierBuildings.value) &&
      (!inventoryBuildingsSearch.value ||
        inventory.item.name
          .toLowerCase()
          .includes(inventoryBuildingsSearch.value.toLowerCase())),
  );
});

const inventoryPlayersSearch = ref<string | null>("");

const inventorysPlayers = computed(() => {
  if (!claimFetch.value?.inventorys?.players?.length) {
    return [];
  }

  if (
    !inventoryPlayersSearch.value &&
    !rarityPlayers.value &&
    !tierPlayers.value
  ) {
    return claimFetch.value?.inventorys?.players;
  }

  return claimFetch.value?.inventorys?.players.filter(
    (inventory) =>
      (!rarityPlayers.value ||
        parseInt(Object.keys(inventory.item.rarity)[0]) ===
          rarityPlayers.value) &&
      (!tierPlayers.value || inventory.item.tier === tierPlayers.value) &&
      (!inventoryPlayersSearch.value ||
        inventory.item.name
          .toLowerCase()
          .includes(inventoryPlayersSearch.value.toLowerCase())),
  );
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
      (!rarityPlayersOffline.value ||
        parseInt(Object.keys(inventory.item.rarity)[0]) ===
          rarityPlayersOffline.value) &&
      (!tierPlayersOffline.value ||
        inventory.item.tier === tierPlayersOffline.value) &&
      (!inventoryPlayersOfflineSearch.value ||
        inventory.item.name
          .toLowerCase()
          .includes(inventoryPlayersOfflineSearch.value.toLowerCase())),
  );
});

const sortMembersLevelRaw = (a: any, b: any) => {
  return b.level - a.level;
};

const theme = useTheme();

const levelToColor = (level: number) => {
  let colorEffect = "";

  if (theme.global.current.value.dark) {
  } else {
    colorEffect = "-darken-4";
  }

  if (1 <= level && level <= 19) {
    return `grey${colorEffect}`;
  }
  if (20 <= level && level <= 29) {
    return `orange${colorEffect}`;
  }
  if (30 <= level && level <= 39) {
    return `green${colorEffect}`;
  }
  if (40 <= level && level <= 49) {
    return `blue${colorEffect}`;
  }
  if (50 <= level && level <= 59) {
    return `purple${colorEffect}`;
  }
  if (60 <= level && level <= 69) {
    return `red${colorEffect}`;
  }
  if (70 <= level && level <= 79) {
    return `yellow${colorEffect}`;
  }
  if (80 <= level && level <= 89) {
    return `teal${colorEffect}`;
  }
  if (90 <= level && level <= 99) {
    return `deepPurple${colorEffect}`;
  }
  if (100 <= level) {
    return `deepPurple${colorEffect}`;
  }
};

const tierToColor = computed(() => {
  let colorEffect = "";

  if (theme.global.current.value.dark) {
  } else {
    colorEffect = "-darken-4";
  }

  const colors = {
    1: `grey${colorEffect}`,
    2: `orange${colorEffect}`,
    3: `green${colorEffect}`,
    4: `blue${colorEffect}`,
    5: `purple${colorEffect}`,
    6: `red${colorEffect}`,
    7: `yellow${colorEffect}`,
    8: `teal${colorEffect}`,
    9: `deepPurple${colorEffect}`,
    10: `deepPurple${colorEffect}`,
  };

  return colors;
});

const length = computed(() => {
  return Math.ceil((buidlingsFetch.value?.total || 0) / perPage) ?? 0;
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
  const weeks = Math.floor(
    (seconds % (60 * 60 * 24 * 365)) / (60 * 60 * 24 * 7),
  );
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

let tab = ref(null);

let memberSearch = ref<string | null>(null);
let showOnlyOnlineMembers = ref(false);

const membersForTable = computed(() => {
  if (!claim.value?.members) {
    return [];
  }
  return Object.values(claim.value.members)
    .filter((member) => {
      if (showOnlyOnlineMembers.value && member.online_state !== "Online") {
        return false;
      }

      if (memberSearch.value) {
        return member.user_name
          .toLowerCase()
          .includes(memberSearch.value.toLowerCase());
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
});

const onlinePlayersCount = computed(() => {
  if (claimFetch.value === undefined) {
    return 0;
  }
  return (
    Object.values(claimFetch.value?.members).filter(
      (member) => member.online_state === "Online",
    ).length ?? 0
  );
});

const now = useNow({ interval: 1000, controls: true });

const researchEnded = computed(() => {
  return new Date(
    new Date(
      claimFetch.value?.running_upgrade_started
        ?.__timestamp_micros_since_unix_epoch__,
    ).getTime() +
      claimFetch.value?.running_upgrade?.research_time * 1000,
  );
});

// Show Days Hours Minutes Seconds
const countDownUntilResearchIsFinished = computed(() => {
  const diff = researchEnded.value.getTime() - now.now.value.getTime();

  return {
    days: Math.floor(diff / (1000 * 60 * 60 * 24)),
    hours: Math.floor((diff / (1000 * 60 * 60)) % 24),
    minutes: Math.floor((diff / 1000 / 60) % 60),
    seconds: Math.floor((diff / 1000) % 60),
  };
});

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
  Slayer: 14,
  Smithing: 4,
  Tailoring: 7,
};

watchThrottled(
  () => [item_object.value, player_id.value],
  (value, oldValue) => {
    InventoryChangelogRefresh();
  },
  { throttle: 50 },
);
</script>

<template>
  <v-container fluid>
    <v-row>
      <v-col cols="12">
        <v-card height="100%" v-if="claim !== undefined">
          <v-card-item>
            <v-card-title class="text-center">
              {{ claim.name }}
            </v-card-title>
          </v-card-item>
          <v-card-text>
            <v-row>
              <v-col cols="6" md="2" lg="1">
                <v-list-item>
                  <v-list-item-title>Owner</v-list-item-title>
                  <v-list-item-subtitle>{{ claimOwner?.user_name ?? '' }}</v-list-item-subtitle>
                </v-list-item>
              </v-col>
              <v-col cols="6" md="2" lg="1">
                <v-list-item>
                  <v-list-item-title>Supplies</v-list-item-title>
                  <v-list-item-subtitle>
                    <bitcraft-animated-number v-if="claim.supplies" :value="claim.supplies"
                                              :speed="50"></bitcraft-animated-number>
                  </v-list-item-subtitle>
                </v-list-item>
              </v-col>
              <v-col cols="6" md="2" lg="1">
                <v-list-item>
                  <v-list-item-title>Tiles</v-list-item-title>
                  <v-list-item-subtitle>
                    <bitcraft-animated-number v-if="claim.num_tiles" :value="claim.num_tiles"
                                              :speed="50"></bitcraft-animated-number>
                  </v-list-item-subtitle>
                </v-list-item>
              </v-col>
              <v-col cols="6" md="2" lg="1">
                <v-list-item>
                  <v-list-item-title>Treasury</v-list-item-title>
                  <v-list-item-subtitle>
                    <bitcraft-animated-number v-if="claim.treasury" :value="claim.treasury"
                                              :speed="50"></bitcraft-animated-number>
                  </v-list-item-subtitle>
                </v-list-item>
              </v-col>
              <v-col cols="6" md="2" lg="1" v-if="claim?.location && claim?.location.x != 0 && claim?.location.z != 0">
                <v-list-item>
                  <v-list-item-title>Location</v-list-item-title>
                  <v-list-item-subtitle>
                    R: {{ claim.region.replace("bitcraft-", "") }} N: {{ Math.ceil(claim.location.z / 3) }}, E:
                    {{ Math.ceil(claim.location.x / 3) }}
                  </v-list-item-subtitle>
                </v-list-item>
              </v-col>
              <v-col cols="6" md="2" lg="1">
                <v-list-item>
                  <v-list-item-title>Buildings</v-list-item-title>
                  <v-list-item-subtitle>
                    {{ claimFetch?.building_states?.length || 0 }}
                  </v-list-item-subtitle>
                </v-list-item>
              </v-col>
              <v-col cols="6" md="2" lg="1">
                <v-list-item>
                  <v-list-item-title>Tier</v-list-item-title>
                  <v-list-item-subtitle>
                    {{ claimFetch?.tier || 1 }}
                  </v-list-item-subtitle>
                </v-list-item>
              </v-col>
              <v-col v-if="claimFetch?.running_upgrade" cols="6" md="2" lg="1">
                <v-list-item>
                  <v-list-item-title>Current Research</v-list-item-title>
                  <v-list-item-subtitle>
                    <strong>{{ claimFetch?.running_upgrade.description }}</strong> is going to be finished at: <strong
                      v-if="countDownUntilResearchIsFinished.days">{{
                      countDownUntilResearchIsFinished.days
                    }}d </strong><strong v-if="countDownUntilResearchIsFinished.hours">{{
                      countDownUntilResearchIsFinished.hours
                    }}h </strong><strong v-if="countDownUntilResearchIsFinished.minutes">{{
                      countDownUntilResearchIsFinished.minutes
                    }}m </strong><strong v-if="countDownUntilResearchIsFinished.seconds">{{
                      countDownUntilResearchIsFinished.seconds
                    }}s</strong>
                  </v-list-item-subtitle>
                </v-list-item>
              </v-col>
              <v-col cols="6" md="2" lg="1">
                <v-list-item>
                  <v-list-item-title>Total time signed in</v-list-item-title>
                  <v-list-item-subtitle>
                    {{ secondsToDaysMinutesSecondsFormat(claimFetch?.time_signed_in) }}
                  </v-list-item-subtitle>
                </v-list-item>
              </v-col>
            </v-row>
          </v-card-text>
        </v-card>
      </v-col>
      <v-col cols="12">
        <v-card height="100%">
          <v-tabs
              v-model="tab"
          >
            <v-tab value="members">Members</v-tab>
            <v-tab value="building_items">Building items ({{ inventorysBuildings.length || 0 }})</v-tab>
            <v-tab value="player_items">Player items ({{ inventorysPlayers.length || 0 }})</v-tab>
            <v-tab value="player_offline_items">Player Offline items ({{
                inventorysPlayersOffline.length || 0
              }})
            </v-tab>
            <v-tab value="buildings">Buildings ({{ buildings.length || 0 }})</v-tab>
            <v-tab value="leaderboards">Leaderboards</v-tab>
            <v-tab value="inventory_changelogs">Inventory Changes ({{ InventoryChangelogFetch?.length || 0 }})</v-tab>
            <v-tab value="traveler_tasks">Traveler Tasks</v-tab>
          </v-tabs>

          <v-card-text>
            <v-tabs-window v-model="tab">
              <v-tabs-window-item value="members">
                <v-card height="100%">
                  <v-card-title class="d-flex align-center pe-2">
                    Members (
                    <div
                        :class="`text-decoration-none ${onlinePlayersCount > 0 ? 'text-green' : 'text-high-emphasis'}`">
                      {{ onlinePlayersCount }}
                    </div>
                    /{{ claimFetch ? Object.values(claimFetch.members).length : 0 }})

                    <v-spacer></v-spacer>
                    <v-checkbox
                        v-model="showOnlyOnlineMembers"
                        label="Show only online members"
                    ></v-checkbox>

                    <v-text-field
                        v-model="memberSearch"
                        density="compact"
                        label="Search"
                        prepend-inner-icon="mdi-magnify"
                        variant="solo-filled"
                        flat
                        hide-details
                        single-line
                    ></v-text-field>
                  </v-card-title>
                  <v-card-text>
                    <v-data-table
                        hover
                        density="compact"
                        :sort-by="[{ key: 'permissions', order: 'desc' }, { key: 'online_state', order: 'desc' }]"
                        :headers="[
                {
                  title: 'User',
                  key: 'user_name',
                  cellProps: {
                    class: 'font-weight-black'
                  }
                },
                 {
                  title: 'Permissions',
                  key: 'permissions',
                },
                  {
                    title: 'Carpentry',
                    key: 'skills_ranks.Carpentry',
                    sort: sortMembersLevelRaw
                  },
                  {
                    title: 'Farming',
                    key: 'skills_ranks.Farming',
                    sort: sortMembersLevelRaw
                  },
                  {
                    title: 'Fishing',
                    key: 'skills_ranks.Fishing',
                    sort: sortMembersLevelRaw
                  },
                  {
                    title: 'Foraging',
                    key: 'skills_ranks.Foraging',
                    sort: sortMembersLevelRaw
                  },
                  {
                    title: 'Forestry',
                    key: 'skills_ranks.Forestry',
                    sort: sortMembersLevelRaw
                  },
                  {
                    title: 'Hunting',
                    key: 'skills_ranks.Hunting',
                    sort: sortMembersLevelRaw
                  },
                  {
                    title: 'Leatherworking',
                    key: 'skills_ranks.Leatherworking',
                    sort: sortMembersLevelRaw
                  },
                  {
                    title: 'Masonry',
                    key: 'skills_ranks.Masonry',
                    sort: sortMembersLevelRaw
                  },
                  {
                    title: 'Mining',
                    key: 'skills_ranks.Mining',
                    sort: sortMembersLevelRaw
                  },
                  {
                    title: 'Scholar',
                    key: 'skills_ranks.Scholar',
                    sort: sortMembersLevelRaw
                  },
                  {
                    title: 'Smithing',
                    key: 'skills_ranks.Smithing',
                    sort: sortMembersLevelRaw
                  },
                  {
                    title: 'Tailoring',
                    key: 'skills_ranks.Tailoring',
                    sort: sortMembersLevelRaw
                  },
                ]"
                        :items="membersForTable"
                        :items-per-page="15"
                        :items-per-page-options="[15, 25 ,50]"
                        class="elevation-1"

                    >
                      <template #item.user_name="{ item }">
                        <nuxt-link
                            :class="`text-decoration-none ${item.online_state === 'Online' ? 'text-green' : 'text-high-emphasis'}`"
                            :to="{ name: 'players-id', params: { id: item.entity_id } }">
                          {{ item.user_name }}
                        </nuxt-link>
                      </template>
                      <template #item.permissions="{ item }">
                        {{ item.co_owner_permission ? "🏰" : "" }}
                        {{ item.officer_permission ? "🗡️" : "" }}
                        {{ item.build_permission ? "🔨" : "" }}
                        {{ item.inventory_permission ? "📦" : "" }}
                      </template>
                      <template #item.skills_ranks.Carpentry="{ value, item }">
                        <div style="white-space: nowrap;">
                          <v-chip class="font-weight-black rounded-e" style="flex-wrap: nowrap;"
                                  :color="levelToColor(value?.level)">{{ value?.level }}
                          </v-chip>
                          <v-chip class="font-weight-black rounded-s" style="flex-wrap: nowrap;"
                                  :color="tierToColor[item?.inventory?.pockets[skillToToolIndex['Carpentry']]?.contents?.item.tier]">
                            <div v-if="item?.inventory?.pockets[skillToToolIndex['Carpentry']]?.contents?.item.tier">
                              T{{ item?.inventory?.pockets[skillToToolIndex['Carpentry']]?.contents?.item.tier }} {{
                                Array.from(item?.inventory?.pockets[skillToToolIndex['Carpentry']]?.contents?.item.rarity)[0]
                              }}
                            </div>
                          </v-chip>
                        </div>
                      </template>
                      <template #item.skills_ranks.Farming="{ value, item }">
                        <div style="white-space: nowrap;">
                          <v-chip class="font-weight-black rounded-e" style="flex-wrap: nowrap;"
                                  :color="levelToColor(value?.level)">{{ value?.level }}
                          </v-chip>
                          <v-chip class="font-weight-black rounded-s" style="flex-wrap: nowrap;"
                                  :color="tierToColor[item?.inventory?.pockets[skillToToolIndex['Farming']]?.contents?.item.tier]">
                            <div v-if="item?.inventory?.pockets[skillToToolIndex['Farming']]?.contents?.item.tier">
                              T{{ item?.inventory?.pockets[skillToToolIndex['Farming']]?.contents?.item.tier }} {{
                                Array.from(item?.inventory?.pockets[skillToToolIndex['Farming']]?.contents?.item.rarity)[0]
                              }}
                            </div>
                          </v-chip>
                        </div>
                      </template>
                      <template #item.skills_ranks.Fishing="{ value, item }">
                        <div style="white-space: nowrap;">
                          <v-chip class="font-weight-black rounded-e" style="flex-wrap: nowrap;"
                                  :color="levelToColor(value?.level)">{{ value?.level }}
                          </v-chip>
                          <v-chip class="font-weight-black rounded-s" style="flex-wrap: nowrap;"
                                  :color="tierToColor[item?.inventory?.pockets[skillToToolIndex['Fishing']]?.contents?.item.tier]">
                            <div v-if="item?.inventory?.pockets[skillToToolIndex['Fishing']]?.contents?.item.tier">
                              T{{ item?.inventory?.pockets[skillToToolIndex['Fishing']]?.contents?.item.tier }} {{
                                Array.from(item?.inventory?.pockets[skillToToolIndex['Fishing']]?.contents?.item.rarity)[0]
                              }}
                            </div>
                          </v-chip>
                        </div>
                      </template>
                      <template #item.skills_ranks.Foraging="{ value, item }">
                        <div style="white-space: nowrap;">
                          <v-chip class="font-weight-black rounded-e" style="flex-wrap: nowrap;"
                                  :color="levelToColor(value?.level)">{{ value?.level }}
                          </v-chip>
                          <v-chip class="font-weight-black rounded-s" style="flex-wrap: nowrap;"
                                  :color="tierToColor[item?.inventory?.pockets[skillToToolIndex['Foraging']]?.contents?.item.tier]">
                            <div v-if="item?.inventory?.pockets[skillToToolIndex['Foraging']]?.contents?.item.tier">
                              T{{ item?.inventory?.pockets[skillToToolIndex['Foraging']]?.contents?.item.tier }} {{
                                Array.from(item?.inventory?.pockets[skillToToolIndex['Foraging']]?.contents?.item.rarity)[0]
                              }}
                            </div>
                          </v-chip>
                        </div>
                      </template>
                      <template #item.skills_ranks.Forestry="{ value, item }">
                        <div style="white-space: nowrap;">
                          <v-chip class="font-weight-black rounded-e" style="flex-wrap: nowrap;"
                                  :color="levelToColor(value?.level)">{{ value?.level }}
                          </v-chip>
                          <v-chip class="font-weight-black rounded-s" style="flex-wrap: nowrap;"
                                  :color="tierToColor[item?.inventory?.pockets[skillToToolIndex['Forestry']]?.contents?.item.tier]">
                            <div v-if="item?.inventory?.pockets[skillToToolIndex['Forestry']]?.contents?.item.tier">
                              T{{ item?.inventory?.pockets[skillToToolIndex['Forestry']]?.contents?.item.tier }} {{
                                Array.from(item?.inventory?.pockets[skillToToolIndex['Forestry']]?.contents?.item.rarity)[0]
                              }}
                            </div>
                          </v-chip>
                        </div>
                      </template>
                      <template #item.skills_ranks.Hunting="{ value, item }">
                        <div style="white-space: nowrap;">
                          <v-chip class="font-weight-black rounded-e" style="flex-wrap: nowrap;"
                                  :color="levelToColor(value?.level)">{{ value?.level }}
                          </v-chip>
                          <v-chip class="font-weight-black rounded-s" style="flex-wrap: nowrap;"
                                  :color="tierToColor[item?.inventory?.pockets[skillToToolIndex['Hunting']]?.contents?.item.tier]">
                            <div v-if="item?.inventory?.pockets[skillToToolIndex['Hunting']]?.contents?.item.tier">
                              T{{ item?.inventory?.pockets[skillToToolIndex['Hunting']]?.contents?.item.tier }} {{
                                Array.from(item?.inventory?.pockets[skillToToolIndex['Hunting']]?.contents?.item.rarity)[0]
                              }}
                            </div>
                          </v-chip>
                        </div>
                      </template>
                      <template #item.skills_ranks.Leatherworking="{ value, item }">
                        <div style="white-space: nowrap;">
                          <v-chip class="font-weight-black rounded-e" style="flex-wrap: nowrap;"
                                  :color="levelToColor(value?.level)">{{ value?.level }}
                          </v-chip>
                          <v-chip class="font-weight-black rounded-s" style="flex-wrap: nowrap;"
                                  :color="tierToColor[item?.inventory?.pockets[skillToToolIndex['Leatherworking']]?.contents?.item.tier]">
                            <div
                                v-if="item?.inventory?.pockets[skillToToolIndex['Leatherworking']]?.contents?.item.tier">
                              T{{ item?.inventory?.pockets[skillToToolIndex['Leatherworking']]?.contents?.item.tier }}
                              {{
                                Array.from(item?.inventory?.pockets[skillToToolIndex['Leatherworking']]?.contents?.item.rarity)[0]
                              }}
                            </div>
                          </v-chip>
                        </div>
                      </template>
                      <template #item.skills_ranks.Masonry="{ value, item }">
                        <div style="white-space: nowrap;">
                          <v-chip class="font-weight-black rounded-e" style="flex-wrap: nowrap;"
                                  :color="levelToColor(value?.level)">{{ value?.level }}
                          </v-chip>
                          <v-chip class="font-weight-black rounded-s" style="flex-wrap: nowrap;"
                                  :color="tierToColor[item?.inventory?.pockets[skillToToolIndex['Masonry']]?.contents?.item.tier]">
                            <div v-if="item?.inventory?.pockets[skillToToolIndex['Masonry']]?.contents?.item.tier">
                              T{{ item?.inventory?.pockets[skillToToolIndex['Masonry']]?.contents?.item.tier }} {{
                                Array.from(item?.inventory?.pockets[skillToToolIndex['Masonry']]?.contents?.item.rarity)[0]
                              }}
                            </div>
                          </v-chip>
                        </div>
                      </template>
                      <template #item.skills_ranks.Mining="{ value, item }">
                        <div style="white-space: nowrap;">
                          <v-chip class="font-weight-black rounded-e" style="flex-wrap: nowrap;"
                                  :color="levelToColor(value?.level)">{{ value?.level }}
                          </v-chip>
                          <v-chip class="font-weight-black rounded-s" style="flex-wrap: nowrap;"
                                  :color="tierToColor[item?.inventory?.pockets[skillToToolIndex['Mining']]?.contents?.item.tier]">
                            <div v-if="item?.inventory?.pockets[skillToToolIndex['Mining']]?.contents?.item.tier">
                              T{{ item?.inventory?.pockets[skillToToolIndex['Mining']]?.contents?.item.tier }} {{
                                Array.from(item?.inventory?.pockets[skillToToolIndex['Mining']]?.contents?.item.rarity)[0]
                              }}
                            </div>
                          </v-chip>
                        </div>
                      </template>
                      <template #item.skills_ranks.Scholar="{ value, item }">
                        <div style="white-space: nowrap;">
                          <v-chip class="font-weight-black rounded-e" style="flex-wrap: nowrap;"
                                  :color="levelToColor(value?.level)">{{ value?.level }}
                          </v-chip>
                          <v-chip class="font-weight-black rounded-s" style="flex-wrap: nowrap;"
                                  :color="tierToColor[item?.inventory?.pockets[skillToToolIndex['Scholar']]?.contents?.item.tier]">
                            <div v-if="item?.inventory?.pockets[skillToToolIndex['Scholar']]?.contents?.item.tier">
                              T{{ item?.inventory?.pockets[skillToToolIndex['Scholar']]?.contents?.item.tier }} {{
                                Array.from(item?.inventory?.pockets[skillToToolIndex['Scholar']]?.contents?.item.rarity)[0]
                              }}
                            </div>
                          </v-chip>
                        </div>
                      </template>
                      <template #item.skills_ranks.Smithing="{ value, item }">
                        <div style="white-space: nowrap;">
                          <v-chip class="font-weight-black rounded-e" style="flex-wrap: nowrap;"
                                  :color="levelToColor(value?.level)">{{ value?.level }}
                          </v-chip>
                          <v-chip class="font-weight-black rounded-s" style="flex-wrap: nowrap;"
                                  :color="tierToColor[item?.inventory?.pockets[skillToToolIndex['Smithing']]?.contents?.item.tier]">
                            <div v-if="item?.inventory?.pockets[skillToToolIndex['Smithing']]?.contents?.item.tier">
                              T{{ item?.inventory?.pockets[skillToToolIndex['Smithing']]?.contents?.item.tier }} {{
                                Array.from(item?.inventory?.pockets[skillToToolIndex['Smithing']]?.contents?.item.rarity)[0]
                              }}
                            </div>
                          </v-chip>
                        </div>
                      </template>
                      <template #item.skills_ranks.Tailoring="{ value, item }">
                        <div style="white-space: nowrap;">
                          <v-chip class="font-weight-black rounded-e" style="flex-wrap: nowrap;"
                                  :color="levelToColor(value?.level)">{{ value?.level }}
                          </v-chip>
                          <v-chip class="font-weight-black rounded-s" style="flex-wrap: nowrap;"
                                  :color="tierToColor[item?.inventory?.pockets[skillToToolIndex['Tailoring']]?.contents?.item.tier]">
                            <div v-if="item?.inventory?.pockets[skillToToolIndex['Tailoring']]?.contents?.item.tier">
                              T{{ item?.inventory?.pockets[skillToToolIndex['Tailoring']]?.contents?.item.tier }} {{
                                Array.from(item?.inventory?.pockets[skillToToolIndex['Tailoring']]?.contents?.item.rarity)[0]
                              }}
                            </div>
                          </v-chip>
                        </div>
                      </template>
                    </v-data-table>
                  </v-card-text>
                </v-card>
              </v-tabs-window-item>

              <v-tabs-window-item value="building_items">
                <v-row>
                  <v-col>
                    <v-text-field
                        v-model="inventoryBuildingsSearch"
                        label="Search"
                        outlined
                        dense
                        clearable
                    ></v-text-field>
                  </v-col>
                  <v-col>
                    <v-autocomplete
                        v-model="tierBuildings"
                        :items="Array.from(new Set(claimFetch?.inventorys?.buildings?.map((inventory) => inventory.item.tier) || [])).sort((a, b) => a - b)"
                        label="Tier"
                        outlined
                        dense
                        clearable
                    ></v-autocomplete>
                  </v-col>
                  <v-col>
                    <v-select
                        v-model="rarityPlayers"
                        :items="Array.from(new Set(claimFetch?.inventorys?.buildings?.map((inventory) => parseInt(Object.keys(inventory.item.rarity)[0])) || [])).sort((a, b) => a - b)"
                        label="Rarity"
                        outlined
                        dense
                        clearable
                    ></v-select>
                  </v-col>
                </v-row>
                <v-row>
                  <v-data-iterator :items="inventorysBuildings" :items-per-page="50">
                    <template v-slot:default="{ items }">
                      <v-row>
                        <template
                            v-for="(inventory, i) in items"
                            :key="i"
                        >
                          <v-col cols="12" md="4" lg="3" xl="2">
                            <v-list-item>
                              <template #prepend v-if="iconDomain">
                                <v-avatar :rounded="false" size="50" style="width: 90px;">
                                  <v-img :cover="false"
                                         :src="iconAssetUrlNameRandom(inventory.raw.item.icon_asset_name).url"></v-img>
                                </v-avatar>
                              </template>
                              <div :class="`text-${tierToColor[inventory.raw.item.tier]}`">
                                {{ inventory.raw.item.name }}:
                                <strong>{{ inventory.raw.quantity }}</strong>
                              </div>
                            </v-list-item>
                          </v-col>
                        </template>
                      </v-row>
                    </template>
                  </v-data-iterator>
                </v-row>
              </v-tabs-window-item>

              <v-tabs-window-item value="player_items">
                <v-row>
                  <v-col>
                    <v-text-field
                        v-model="inventoryPlayersSearch"
                        label="Search"
                        outlined
                        dense
                        clearable
                    ></v-text-field>
                  </v-col>
                  <v-col>
                    <v-autocomplete
                        v-model="tierPlayers"
                        :items="Array.from(new Set(claimFetch?.inventorys?.players?.map((inventory) => inventory.item.tier) || [])).sort((a, b) => a - b)"
                        label="Tier"
                        outlined
                        dense
                        clearable
                    ></v-autocomplete>
                  </v-col>
                  <v-col>
                    <v-select
                        v-model="rarityPlayers"
                        :items="Array.from(new Set(claimFetch?.inventorys?.players?.map((inventory) => parseInt(Object.keys(inventory.item.rarity)[0])) || [])).sort((a, b) => a - b)"
                        label="Rarity"
                        outlined
                        dense
                        clearable
                    ></v-select>
                  </v-col>
                </v-row>
                <v-row>
                  <v-data-iterator :items="inventorysPlayers" :items-per-page="50">
                    <template v-slot:default="{ items }">
                      <v-row>
                        <template
                            v-for="(inventory, i) in items"
                            :key="i"
                        >
                          <v-col cols="12" md="4" lg="3" xl="2">
                            <v-list-item>
                              <template #prepend v-if="iconDomain">
                                <v-avatar :rounded="false" size="50" style="width: 90px;">
                                  <v-img :cover="false"
                                         :src="iconAssetUrlNameRandom(inventory.raw.item.icon_asset_name).url"></v-img>
                                </v-avatar>
                              </template>
                              <div :class="`text-${tierToColor[inventory.raw.item.tier]}`">
                                {{ inventory.raw.item.name }}:
                                <strong>{{ inventory.raw.quantity }}</strong>
                              </div>
                            </v-list-item>
                          </v-col>
                        </template>
                      </v-row>
                    </template>
                  </v-data-iterator>
                </v-row>
              </v-tabs-window-item>

              <v-tabs-window-item value="player_offline_items">
                <v-row>
                  <v-col>
                    <v-text-field
                        v-model="inventoryPlayersOfflineSearch"
                        label="Search"
                        outlined
                        dense
                        clearable
                    ></v-text-field>
                  </v-col>
                  <v-col>
                    <v-autocomplete
                        v-model="tierPlayersOffline"
                        :items="Array.from(new Set(claimFetch?.inventorys?.players_offline?.map((inventory) => inventory.item.tier) || [])).sort((a, b) => a - b)"
                        label="Tier"
                        outlined
                        dense
                        clearable
                    ></v-autocomplete>
                  </v-col>
                  <v-col>
                    <v-select
                        v-model="rarityPlayersOffline"
                        :items="Array.from(new Set(claimFetch?.inventorys?.players_offline?.map((inventory) => parseInt(Object.keys(inventory.item.rarity)[0])) || [])).sort((a, b) => a - b)"
                        label="Rarity"
                        outlined
                        dense
                        clearable
                    ></v-select>
                  </v-col>
                </v-row>
                <v-row>
                  <v-data-iterator :items="inventorysPlayersOffline" :items-per-page="50">
                    <template v-slot:default="{ items }">
                      <v-row>
                        <template
                            v-for="(inventory, i) in items"
                            :key="i"
                        >
                          <v-col cols="12" md="4" lg="3" xl="2">
                            <v-list-item>
                              <template #prepend v-if="iconDomain">
                                <v-avatar :rounded="false" size="50" style="width: 90px;">
                                  <v-img :cover="false"
                                         :src="iconAssetUrlNameRandom(inventory.raw.item.icon_asset_name).url"></v-img>
                                </v-avatar>
                              </template>
                              <div :class="`text-${tierToColor[inventory.raw.item.tier]}`">
                                {{ inventory.raw.item.name }}:
                                <strong>{{ inventory.raw.quantity }}</strong>
                              </div>
                            </v-list-item>
                          </v-col>
                        </template>
                      </v-row>
                    </template>
                  </v-data-iterator>
                </v-row>
              </v-tabs-window-item>
              <v-tabs-window-item value="buildings">
                <v-col>
                  <v-text-field
                      v-model="search"
                      label="Search"
                      outlined
                      dense
                      clearable
                  ></v-text-field>
                </v-col>
                <v-row>
                  <v-col>
                    <v-progress-linear
                        color="yellow-darken-2"
                        indeterminate
                        :active="buildingsPending"
                    ></v-progress-linear>
                  </v-col>
                </v-row>
                <v-row>
                  <v-col cols="12" md="4" lg="3" xl="2" v-for="building in buildings" :key="building.entity_id">
                    <nuxt-link :to="{ name: 'buildings-id', params: { id: building.entity_id.toString() } }"
                               class="text-high-emphasis font-weight-black">
                      <v-list-item>
                        <template #prepend v-if="iconDomain">
                          <v-avatar :image="`${iconDomain}/${building.image_path}`" size="50"></v-avatar>
                        </template>
                        {{ building.building_name }}
                      </v-list-item>
                    </nuxt-link>
                  </v-col>
                </v-row>
                <v-row>
                  <v-col>
                    <v-pagination
                        v-model="page"
                        :length="length"
                    ></v-pagination>
                  </v-col>
                </v-row>
              </v-tabs-window-item>
              <v-tabs-window-item value="leaderboards">
                <leaderboard-claim :claim-id="claim?.entity_id"></leaderboard-claim>
              </v-tabs-window-item>
              <v-tabs-window-item value="inventory_changelogs">
                <v-card>
                  <v-card-title>Changes</v-card-title>
                  <v-card-text>
                    <v-row>
                      <v-col>
                        <autocomplete-user
                            @model_changed="(item) => player_id=item"
                        />
                      </v-col>
                      <v-col>
                        <autocomplete-item
                            @model_changed="(item) => item_object=item"
                        />
                      </v-col>
                    </v-row>
                    <inventory-changes :items="InventoryChangelogFetch"/>
                  </v-card-text>
                </v-card>
              </v-tabs-window-item>
              <v-tabs-window-item value="traveler_tasks">
                <v-data-table
                    hover
                    density="compact"
                    :headers="[
                {
                  title: 'Items',
                  key: 'items',
                  cellProps: {
                    class: 'font-weight-black'
                  }
                },
                {
                  title: 'Name',
                  key: 'name',
                  cellProps: {
                    class: 'font-weight-black'
                  }
                },
                 {
                  title: 'NPC Name',
                  key: 'npc_name',
                  cellProps: {
                    class: 'font-weight-black'
                  }
                },
                 {
                  title: 'Player Count',
                  key: 'player_count',
                  cellProps: {
                    class: 'font-weight-black'
                  }
                },
                {
                  title: 'User Names',
                  key: 'users',
                  cellProps: {
                    class: 'font-weight-black'
                  }
                },
                ]"
                    :items="Object.entries(claimFetch?.traveler_tasks?.players) || {}"
                    :items-per-page="15"
                    class="elevation-1"

                >
                  <template #item.items="{ item }">
                    <template v-for="shownItem of trevelerTasksFetch[item[0]]?.required_items ">
                      <v-badge :content="Intl.NumberFormat().format(shownItem.quantity)" location="right"
                               class="align-start">
                        <template v-if="shownItem.item_type == 'Item'">
                          <v-img
                              :src="iconAssetUrlNameRandom(itemsAndCargoAllFetch.item_desc[shownItem.item_id].icon_asset_name).url"
                              height="75" :width="shownItem.type == 'Item' ? 75 : 128"></v-img>
                        </template>
                        <template v-else-if="shownItem.item_type == 'Cargo'">
                          <v-img
                              :src="iconAssetUrlNameRandom(itemsAndCargoAllFetch.cargo_desc[shownItem.item_id].icon_asset_name).url"
                              height="75" :width="shownItem.type == 'Item' ? 75 : 128"></v-img>
                        </template>
                      </v-badge>
                    </template>
                  </template>
                  <template #item.name="{ item }">
                    <template v-for="shownItem of trevelerTasksFetch[item[0]]?.required_items ">
                      <div class="align-center"
                           :class="`text-${tierToColor[shownItem.item_type == 'Item' ? itemsAndCargoAllFetch.item_desc[shownItem.item_id].tier : itemsAndCargoAllFetch.cargo_desc[shownItem.item_id].tier]}`">
                        <template v-if="shownItem.item_type == 'Item'">
                          {{ itemsAndCargoAllFetch.item_desc[shownItem.item_id].name }}
                        </template>
                        <template v-else-if="shownItem.item_type == 'Cargo'">
                          {{ itemsAndCargoAllFetch.cargo_desc[shownItem.item_id].name }}
                        </template>
                      </div>
                    </template>
                  </template>

                  <template #item.npc_name="{ value, item }">
                    {{ trevelerTasksFetch[item[0]].description.split(" ")[0] }}
                  </template>
                  <template #item.player_count="{ value, item }">
                    {{ item[1].length }}
                  </template>
                  <template #item.users="{ value, item }">
                    <template v-for="playerId of item[1]">
                      <nuxt-link :class="`text-decoration-none`" :to="{ name: 'players-id', params: { id: playerId } }">
                        {{ claimFetch.members[playerId]?.user_name }}
                      </nuxt-link>
                      ,
                    </template>
                  </template>
                </v-data-table>
              </v-tabs-window-item>
            </v-tabs-window>
          </v-card-text>
        </v-card>
      </v-col>
      <v-col cols="12">
      </v-col>
    </v-row>
  </v-container>
</template>
