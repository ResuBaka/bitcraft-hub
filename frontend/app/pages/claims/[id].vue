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

const {
  public: { iconDomain },
} = useRuntimeConfig();

const page = ref(1);
const perPage = 500;

const building_items_collapsible = ref([]);
const player_items_collapsible = ref([]);
const player_offline_items_collapsible = ref([]);
const buildings_collapsible = ref([]);
const inventory_changelog_collapsible = ref([]);

const search = ref<string | null>("");

const route = useRoute();
const router = useRouter();

const player_id = ref<BigInt | null>();
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
const {
  public: { api },
} = useRuntimeConfig();

const { data: claimFetch, pending: claimPnding } =
  useFetchMsPack<ClaimDescriptionStateWithInventoryAndPlayTime>(() => {
    return `${api.base}/api/bitcraft/claims/${route.params.id.toString()}`;
  });

const { data: InventoryChangelogFetch, refresh: InventoryChangelogRefresh } = useFetchMsPack<InventoryChangelog[]>(
  () => {
    return `${api.base}/claims/inventory_changelog/${route.params.id.toString()}`
  },{
    onRequest: ({ options }) => {
        options.query = options.query || {};
        console.log(item_object.value)
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
      }
  }
);

const { data: buidlingsFetch, pending: buildingsPending } =
  useFetchMsPack<BuildingStatesResponse>(() => {
    return `${api.base}/api/bitcraft/buildings?claim_entity_id=${route.params.id}&with_inventory=true&page=${page.value}&per_page=${perPage}`;
  });

const topicsPlayer = computed<string[]>(() => {
  return (
    claimFetch.value?.members.map((member) => {
      return `player_state.${member.entity_id}`;
    }) ?? []
  );
});

registerWebsocketMessageHandler(
  "ClaimLocalState",
  [`claim_local_state.${route.params.id}`],
  (message) => {
    if (message.c.entity_id == route.params.id) {
      if (claimFetch.value) {
        claimFetch.value.name = message.c.name;
        claimFetch.value.num_tiles = message.c.num_tiles;
        claimFetch.value.owner_player_entity_id =
          message.c.owner_player_entity_id;
        claimFetch.value.supplies = message.c.supplies;
        claimFetch.value.treasury = message.c.treasury;
        claimFetch.value.xp_gained_since_last_coin_minting =
          message.c.xp_gained_since_last_coin_minting;
      }
    }
  },
);

registerWebsocketMessageHandler("PlayerState", topicsPlayer, (message) => {
  let index = claimFetch.value?.members.findIndex(
    (member) => member.entity_id == message.c.entity_id,
  );
  if (index && index !== -1) {
    let onlineState = message.c.signed_in ? "Online" : "Offline";

    if (claimFetch.value.members[index].online_state !== onlineState) {
      if (message.c.signed_in) {
        toast(`${claimFetch.value.members[index].user_name} signed in`, {
          progressBar: true,
          duration: 5000,
        });
      } else {
        toast(`${claimFetch.value.members[index].user_name} signed out`, {
          progressBar: true,
          duration: 5000,
        });
      }
    }

    claimFetch.value.members[index].online_state = onlineState;
  }
});

const topicsLevel = computed<string[]>(() => {
  return (
    claimFetch.value?.members
      .filter((member) => member.online_state === "Online")
      .map((member) => {
        return `level.${member.entity_id}`;
      }) ?? []
  );
});

registerWebsocketMessageHandler("Level", topicsLevel, (message) => {
  let index = claimFetch.value?.members.findIndex(
    (member) => member.entity_id == message.c.user_id,
  );
  if (index && index !== -1) {
    toast(
      `Player ${claimFetch.value.members[index].user_name} Level ${message.c.level} reached for Skill ${message.c.skill_name}`,
      { progressBar: true, duration: 5000 },
    );

    claimFetch.value.members[index].skills_ranks[message.c.skill_name].level =
      message.c.level;
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

const tierColor = function (tier: number) {
  let colorEffect = "";

  if (theme.global.current.value.dark) {
  } else {
    colorEffect = "-darken-4";
  }

  const colors = {
    1: `grey${colorEffect}`,
    2: `green${colorEffect}`,
    3: `blue${colorEffect}`,
    4: `purple${colorEffect}`,
    5: `yellow${colorEffect}`,
    6: `pink${colorEffect}`,
  };

  return colors[tier] ?? "";
};

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
    return `green${colorEffect}`;
  }
  if (30 <= level && level <= 39) {
    return `blue${colorEffect}`;
  }
  if (40 <= level && level <= 49) {
    return `purple${colorEffect}`;
  }
  if (50 <= level && level <= 59) {
    return `yellow${colorEffect}`;
  }
  if (60 <= level && level <= 69) {
    return `orange${colorEffect}`;
  }
  if (70 <= level) {
    return `red${colorEffect}`;
  }
};

const tierToColor = (tier: number) => {
  let colorEffect = "";

  if (theme.global.current.value.dark) {
  } else {
    colorEffect = "-darken-4";
  }
  if (tier == 2) {
    return `green${colorEffect}`;
  }
  if (tier == 3) {
    return `blue${colorEffect}`;
  }
  if (tier == 4) {
    return `purple${colorEffect}`;
  }
  if (tier == 5) {
    return `yellow${colorEffect}`;
  }
  if (tier == 6) {
    return `orange${colorEffect}`;
  }
  if (tier == 7) {
    return `red${colorEffect}`;
  } else {
    return `grey${colorEffect}`;
  }
};

const length = computed(() => {
  return Math.ceil((buidlingsFetch.value?.total || 0) / perPage) ?? 0;
});

const claimOwner = computed(() => {
  if (claim.value === undefined) {
    return "";
  }

  return claim.value.members.find(
    (member) =>
      member.entity_id.toString() ===
      claim.value.owner_player_entity_id?.toString(),
  );
});

const sortedUsersByPermissionLevel = computed(() => {
  if (claim.value === undefined) {
    return [];
  }

  return claim.value.members.sort((a, b) => {
    if (a.entity_id === claim.value.owner_player_entity_id) {
      return -1;
    }
    if (b.entity_id === claim.value.owner_player_entity_id) {
      return 1;
    }
    if (a.co_owner_permission && !b.co_owner_permission) {
      return -1;
    }
    if (b.co_owner_permission && !a.co_owner_permission) {
      return 1;
    }
    if (a.officer_permission && !b.officer_permission) {
      return -1;
    }
    if (b.officer_permission && !a.officer_permission) {
      return 1;
    }
    if (a.build_permission && !b.build_permission) {
      return -1;
    }
    if (b.build_permission && !a.build_permission) {
      return 1;
    }
    if (a.inventory_permission && !b.inventory_permission) {
      return -1;
    }
    if (b.inventory_permission && !a.inventory_permission) {
      return 1;
    }
    return 0;
  });
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

let memberSearch = ref<string | null>(null);
let showOnlyOnlineMembers = ref(false);

const membersForTable = computed(() => {
  if (!claim.value?.members) {
    return [];
  }
  return claim.value.members
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

const nDate = Intl.DateTimeFormat(undefined, {
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  hour12: false,
});

const upgradeWillFinishAt = computed(() => {
  return new Date(
    claimFetch.value?.running_upgrade?.research_time * 1000 +
      claimFetch.value?.running_upgrade_started / 1000,
  );
});

const onlinePlayersCount = computed(() => {
  return (
    claimFetch.value?.members.filter(
      (member) => member.online_state === "Online",
    ).length ?? 0
  );
});

const now = useNow({ interval: 1000, controls: true });

// Show Days Hours Minutes Seconds
const countDownUntilResearchIsFinished = computed(() => {
  const researchEnded = new Date(
    claimFetch.value?.running_upgrade?.research_time * 1000 +
      claimFetch.value?.running_upgrade_started / 1000,
  );
  const diff = researchEnded.getTime() - now.now.value.getTime();

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
      <v-col cols="12" lg="2">
        <v-card height="100%" v-if="claim !== undefined">
          <v-card-item>
            <v-card-title>
              {{ claim.name }}
            </v-card-title>
          </v-card-item>
          <v-card-text>
            <v-row>
              <v-col cols="6" md="2" lg="12">
                <v-list-item>
                  <v-list-item-title>Owner</v-list-item-title>
                  <v-list-item-subtitle>{{ claimOwner?.user_name ?? '' }}</v-list-item-subtitle>
                </v-list-item>
              </v-col>
              <v-col cols="6" md="2" lg="12">
                <v-list-item>
                  <v-list-item-title>Supplies</v-list-item-title>
                  <v-list-item-subtitle>{{ claim.supplies }}</v-list-item-subtitle>
                </v-list-item>
              </v-col>
              <v-col cols="6" md="2" lg="12">
                <v-list-item>
                  <v-list-item-title>Tiles</v-list-item-title>
                  <v-list-item-subtitle>{{ claim.num_tiles }}</v-list-item-subtitle>
                </v-list-item>
              </v-col>
              <v-col cols="6" md="2" lg="12">
                <v-list-item>
                  <v-list-item-title>Treasury</v-list-item-title>
                  <v-list-item-subtitle>{{ claim.treasury }}</v-list-item-subtitle>
                </v-list-item>
              </v-col>
              <v-col cols="6" md="2" lg="12">
                <v-list-item>
                  <v-list-item-title>Current xp for minting</v-list-item-title>
                  <v-list-item-subtitle><bitcraft-animated-number :value="claim.xp_gained_since_last_coin_minting" :speed="8"></bitcraft-animated-number> / 2500</v-list-item-subtitle>
                </v-list-item>
              </v-col>
              <v-col cols="6" md="2" lg="12" v-if="claim?.location && claim?.location.x != 0 && claim?.location.z != 0">
                <v-list-item>
                  <v-list-item-title>Location</v-list-item-title>
                  <v-list-item-subtitle>
                    N: {{ Math.ceil(claim.location.z / 3) }}, E:
                    {{ Math.ceil(claim.location.x / 3) }}
                  </v-list-item-subtitle>
                </v-list-item>
              </v-col>
              <v-col cols="6" md="2" lg="12">
                <v-list-item>
                  <v-list-item-title>Buildings</v-list-item-title>
                  <v-list-item-subtitle>
                    {{ claimFetch?.building_states?.length || 0 }}
                  </v-list-item-subtitle>
                </v-list-item>
              </v-col>
              <v-col cols="6" md="2" lg="12">
                <v-list-item>
                  <v-list-item-title>Tier</v-list-item-title>
                  <v-list-item-subtitle>
                    {{ claimFetch?.tier || 1 }}
                  </v-list-item-subtitle>
                </v-list-item>
              </v-col>
              <v-col v-if="claimFetch?.running_upgrade" cols="6" md="2" lg="12">
                <v-list-item>
                  <v-list-item-title>Current Research</v-list-item-title>
                  <v-list-item-subtitle>
                    <strong>{{ claimFetch?.running_upgrade.description }}</strong> is going to be finished at: <strong v-if="countDownUntilResearchIsFinished.days">{{ countDownUntilResearchIsFinished.days }}d </strong><strong v-if="countDownUntilResearchIsFinished.hours">{{ countDownUntilResearchIsFinished.hours }}h </strong><strong v-if="countDownUntilResearchIsFinished.minutes">{{ countDownUntilResearchIsFinished.minutes }}m </strong><strong v-if="countDownUntilResearchIsFinished.seconds">{{ countDownUntilResearchIsFinished.seconds }}s</strong>
                  </v-list-item-subtitle>
                </v-list-item>
              </v-col>
              <v-col cols="6" md="2" lg="12">
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
      <v-col cols="12" lg="10">
        <v-card height="100%">
          <v-card-title class="d-flex align-center pe-2">
            Members (<div :class="`text-decoration-none ${onlinePlayersCount > 0 ? 'text-green' : 'text-high-emphasis'}`">{{ onlinePlayersCount }}</div>/{{ claim?.members?.length || 0 }})

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
                class="elevation-1"

            >
              <template #item.user_name="{ item }">
                <nuxt-link :class="`text-decoration-none ${item.online_state === 'Online' ? 'text-green' : 'text-high-emphasis'}`" :to="{ name: 'players-id', params: { id: item.entity_id } }">
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
                  <v-chip class="font-weight-black rounded-e" style="flex-wrap: nowrap;" :color="levelToColor(value.level)">{{ value.level}}</v-chip>
                  <v-chip class="font-weight-black rounded-s" style="flex-wrap: nowrap;" :color="tierToColor(item?.inventory?.pockets[skillToToolIndex['Carpentry']]?.contents?.item.tier)"><div v-if="item?.inventory?.pockets[skillToToolIndex['Carpentry']]?.contents?.item.tier">T{{item?.inventory?.pockets[skillToToolIndex['Carpentry']]?.contents?.item.tier}} {{ Array.from(item?.inventory?.pockets[skillToToolIndex['Carpentry']]?.contents?.item.rarity)[0] }} </div></v-chip>
                </div>
              </template>
              <template #item.skills_ranks.Farming="{ value, item }">
                <div style="white-space: nowrap;">
                  <v-chip class="font-weight-black rounded-e" style="flex-wrap: nowrap;" :color="levelToColor(value.level)">{{ value.level}}</v-chip>
                  <v-chip class="font-weight-black rounded-s" style="flex-wrap: nowrap;" :color="tierToColor(item?.inventory?.pockets[skillToToolIndex['Farming']]?.contents?.item.tier)"><div v-if="item?.inventory?.pockets[skillToToolIndex['Farming']]?.contents?.item.tier">T{{item?.inventory?.pockets[skillToToolIndex['Farming']]?.contents?.item.tier}} {{ Array.from(item?.inventory?.pockets[skillToToolIndex['Farming']]?.contents?.item.rarity)[0] }} </div></v-chip>
                </div>
              </template>
              <template #item.skills_ranks.Fishing="{ value, item }">
                <div style="white-space: nowrap;">
                  <v-chip class="font-weight-black rounded-e" style="flex-wrap: nowrap;" :color="levelToColor(value.level)">{{ value.level}}</v-chip>
                  <v-chip class="font-weight-black rounded-s" style="flex-wrap: nowrap;" :color="tierToColor(item?.inventory?.pockets[skillToToolIndex['Fishing']]?.contents?.item.tier)"><div v-if="item?.inventory?.pockets[skillToToolIndex['Fishing']]?.contents?.item.tier">T{{item?.inventory?.pockets[skillToToolIndex['Fishing']]?.contents?.item.tier}} {{ Array.from(item?.inventory?.pockets[skillToToolIndex['Fishing']]?.contents?.item.rarity)[0] }} </div></v-chip>
                </div>
              </template>
              <template #item.skills_ranks.Foraging="{ value, item }">
                <div style="white-space: nowrap;">
                  <v-chip class="font-weight-black rounded-e" style="flex-wrap: nowrap;" :color="levelToColor(value.level)">{{ value.level}}</v-chip>
                  <v-chip class="font-weight-black rounded-s" style="flex-wrap: nowrap;" :color="tierToColor(item?.inventory?.pockets[skillToToolIndex['Foraging']]?.contents?.item.tier)"><div v-if="item?.inventory?.pockets[skillToToolIndex['Foraging']]?.contents?.item.tier">T{{item?.inventory?.pockets[skillToToolIndex['Foraging']]?.contents?.item.tier}} {{ Array.from(item?.inventory?.pockets[skillToToolIndex['Foraging']]?.contents?.item.rarity)[0] }} </div></v-chip>
                </div>
              </template>
              <template #item.skills_ranks.Forestry="{ value, item }">
                <div style="white-space: nowrap;">
                  <v-chip class="font-weight-black rounded-e" style="flex-wrap: nowrap;" :color="levelToColor(value.level)">{{ value.level}}</v-chip>
                  <v-chip class="font-weight-black rounded-s" style="flex-wrap: nowrap;" :color="tierToColor(item?.inventory?.pockets[skillToToolIndex['Forestry']]?.contents?.item.tier)"><div v-if="item?.inventory?.pockets[skillToToolIndex['Forestry']]?.contents?.item.tier">T{{item?.inventory?.pockets[skillToToolIndex['Forestry']]?.contents?.item.tier}} {{ Array.from(item?.inventory?.pockets[skillToToolIndex['Forestry']]?.contents?.item.rarity)[0] }} </div></v-chip>
                </div>
              </template>
              <template #item.skills_ranks.Hunting="{ value, item }">
                <div style="white-space: nowrap;">
                  <v-chip class="font-weight-black rounded-e" style="flex-wrap: nowrap;" :color="levelToColor(value.level)">{{ value.level}}</v-chip>
                  <v-chip class="font-weight-black rounded-s" style="flex-wrap: nowrap;" :color="tierToColor(item?.inventory?.pockets[skillToToolIndex['Hunting']]?.contents?.item.tier)"><div v-if="item?.inventory?.pockets[skillToToolIndex['Hunting']]?.contents?.item.tier">T{{item?.inventory?.pockets[skillToToolIndex['Hunting']]?.contents?.item.tier}} {{ Array.from(item?.inventory?.pockets[skillToToolIndex['Hunting']]?.contents?.item.rarity)[0] }} </div></v-chip>
                </div>
              </template>
              <template #item.skills_ranks.Leatherworking="{ value, item }">
                <div style="white-space: nowrap;">
                  <v-chip class="font-weight-black rounded-e" style="flex-wrap: nowrap;" :color="levelToColor(value.level)">{{ value.level}}</v-chip>
                  <v-chip class="font-weight-black rounded-s" style="flex-wrap: nowrap;" :color="tierToColor(item?.inventory?.pockets[skillToToolIndex['Hunting']]?.contents?.item.tier)"><div v-if="item?.inventory?.pockets[skillToToolIndex['Hunting']]?.contents?.item.tier">T{{item?.inventory?.pockets[skillToToolIndex['Hunting']]?.contents?.item.tier}} {{ Array.from(item?.inventory?.pockets[skillToToolIndex['Hunting']]?.contents?.item.rarity)[0] }} </div></v-chip>
                </div>
              </template>
              <template #item.skills_ranks.Masonry="{ value, item }">
                <div style="white-space: nowrap;">
                  <v-chip class="font-weight-black rounded-e" style="flex-wrap: nowrap;" :color="levelToColor(value.level)">{{ value.level}}</v-chip>
                  <v-chip class="font-weight-black rounded-s" style="flex-wrap: nowrap;" :color="tierToColor(item?.inventory?.pockets[skillToToolIndex['Masonry']]?.contents?.item.tier)"><div v-if="item?.inventory?.pockets[skillToToolIndex['Masonry']]?.contents?.item.tier">T{{item?.inventory?.pockets[skillToToolIndex['Masonry']]?.contents?.item.tier}} {{ Array.from(item?.inventory?.pockets[skillToToolIndex['Masonry']]?.contents?.item.rarity)[0] }} </div></v-chip>
                </div>
              </template>
              <template #item.skills_ranks.Mining="{ value, item }">
                <div style="white-space: nowrap;">
                  <v-chip class="font-weight-black rounded-e" style="flex-wrap: nowrap;" :color="levelToColor(value.level)">{{ value.level}}</v-chip>
                  <v-chip class="font-weight-black rounded-s" style="flex-wrap: nowrap;" :color="tierToColor(item?.inventory?.pockets[skillToToolIndex['Mining']]?.contents?.item.tier)"><div v-if="item?.inventory?.pockets[skillToToolIndex['Mining']]?.contents?.item.tier">T{{item?.inventory?.pockets[skillToToolIndex['Mining']]?.contents?.item.tier}} {{ Array.from(item?.inventory?.pockets[skillToToolIndex['Mining']]?.contents?.item.rarity)[0] }} </div></v-chip>
                </div>
              </template>
              <template #item.skills_ranks.Scholar="{ value, item }">
                <div style="white-space: nowrap;">
                  <v-chip class="font-weight-black rounded-e" style="flex-wrap: nowrap;" :color="levelToColor(value.level)">{{ value.level}}</v-chip>
                  <v-chip class="font-weight-black rounded-s" style="flex-wrap: nowrap;" :color="tierToColor(item?.inventory?.pockets[skillToToolIndex['Scholar']]?.contents?.item.tier)"><div v-if="item?.inventory?.pockets[skillToToolIndex['Scholar']]?.contents?.item.tier">T{{item?.inventory?.pockets[skillToToolIndex['Scholar']]?.contents?.item.tier}} {{ Array.from(item?.inventory?.pockets[skillToToolIndex['Scholar']]?.contents?.item.rarity)[0] }} </div></v-chip>
                </div>
              </template>
              <template #item.skills_ranks.Smithing="{ value, item }">
                <div style="white-space: nowrap;">
                  <v-chip class="font-weight-black rounded-e" style="flex-wrap: nowrap;" :color="levelToColor(value.level)">{{ value.level}}</v-chip>
                  <v-chip class="font-weight-black rounded-s" style="flex-wrap: nowrap;" :color="tierToColor(item?.inventory?.pockets[skillToToolIndex['Smithing']]?.contents?.item.tier)"><div v-if="item?.inventory?.pockets[skillToToolIndex['Smithing']]?.contents?.item.tier">T{{item?.inventory?.pockets[skillToToolIndex['Smithing']]?.contents?.item.tier}} {{ Array.from(item?.inventory?.pockets[skillToToolIndex['Smithing']]?.contents?.item.rarity)[0] }} </div></v-chip>
                </div>
              </template>
              <template #item.skills_ranks.Tailoring="{ value, item }">
                <div style="white-space: nowrap;">
                  <v-chip class="font-weight-black rounded-e" style="flex-wrap: nowrap;" :color="levelToColor(value.level)">{{ value.level}}</v-chip>
                  <v-chip class="font-weight-black rounded-s" style="flex-wrap: nowrap;" :color="tierToColor(item?.inventory?.pockets[skillToToolIndex['Tailoring']]?.contents?.item.tier)"><div v-if="item?.inventory?.pockets[skillToToolIndex['Tailoring']]?.contents?.item.tier">T{{item?.inventory?.pockets[skillToToolIndex['Tailoring']]?.contents?.item.tier}} {{ Array.from(item?.inventory?.pockets[skillToToolIndex['Tailoring']]?.contents?.item.rarity)[0] }} </div></v-chip>
                </div>
              </template>
            </v-data-table>
          </v-card-text>
        </v-card>
      </v-col>
    </v-row>
    <v-row>
      <v-col cols="12">
        <v-expansion-panels v-model="building_items_collapsible">
          <v-expansion-panel value="items">
            <v-expansion-panel-title>
              <v-row>
                <v-col class="d-flex justify-center">
                  <h2 class="pl-md-3 pl-xl-0">Building items ({{ inventorysBuildings.length || 0 }})</h2>
                </v-col>
              </v-row>
            </v-expansion-panel-title>
            <v-expansion-panel-text>
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
                <v-col cols="12" md="4" lg="3" xl="2" v-for="inventory in inventorysBuildings" :key="`buildings-${inventory.item_id}-${inventory.quantity}`">
                  <v-list-item>
                    <template #prepend v-if="iconDomain">
                      <v-avatar :rounded="false" size="50" style="width: 90px;">
                        <v-img :cover="false" :src="iconAssetUrlNameRandom(inventory.item.icon_asset_name).url"></v-img>
                      </v-avatar>
                    </template>
                    <div :class="`text-${tierColor(inventory.item.tier)}`">
                      {{ inventory.item.name }}:
                      <strong>{{ inventory.quantity }}</strong>
                    </div>
                  </v-list-item>
                </v-col>
              </v-row>
            </v-expansion-panel-text>
          </v-expansion-panel>
        </v-expansion-panels>
      </v-col>
    </v-row>
    <v-row>
      <v-col cols="12">
        <v-expansion-panels v-model="player_items_collapsible">
          <v-expansion-panel value="items">
            <v-expansion-panel-title>
              <v-row>
                <v-col class="d-flex justify-center">
                  <h2 class="pl-md-3 pl-xl-0">Player items ({{ inventorysPlayers.length || 0 }})</h2>
                </v-col>
              </v-row>
            </v-expansion-panel-title>
            <v-expansion-panel-text>
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
                <v-col cols="12" md="4" lg="3" xl="2" v-for="inventory in inventorysPlayers" :key="`players-${inventory.item_id}-${inventory.quantity}`">
                  <v-list-item>
                    <template #prepend v-if="iconDomain">
                      <v-avatar :rounded="false" size="50" style="width: 90px;">
                        <v-img :cover="false" :src="iconAssetUrlNameRandom(inventory.item.icon_asset_name).url"></v-img>
                      </v-avatar>
                    </template>
                    <div :class="`text-${tierColor(inventory.item.tier)}`">
                      {{ inventory.item.name }}:
                      <strong>{{ inventory.quantity }}</strong>
                    </div>
                  </v-list-item>
                </v-col>
              </v-row>
            </v-expansion-panel-text>
          </v-expansion-panel>
        </v-expansion-panels>
      </v-col>
    </v-row>
    <v-row>
      <v-col cols="12">
        <v-expansion-panels v-model="player_offline_items_collapsible">
          <v-expansion-panel value="items">
            <v-expansion-panel-title>
              <v-row>
                <v-col class="d-flex justify-center">
                  <h2 class="pl-md-3 pl-xl-0">Player Offline items ({{ inventorysPlayersOffline.length || 0 }})</h2>
                </v-col>
              </v-row>
            </v-expansion-panel-title>
            <v-expansion-panel-text>
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
                <v-col cols="12" md="4" lg="3" xl="2" v-for="inventory in inventorysPlayersOffline" :key="`players-${inventory.item_id}-${inventory.quantity}`">
                  <v-list-item>
                    <template #prepend v-if="iconDomain">
                      <v-avatar :rounded="false" size="50" style="width: 90px;">
                        <v-img :cover="false" :src="iconAssetUrlNameRandom(inventory.item.icon_asset_name).url"></v-img>
                      </v-avatar>
                    </template>
                    <div :class="`text-${tierColor(inventory.item.tier)}`">
                      {{ inventory.item.name }}:
                      <strong>{{ inventory.quantity }}</strong>
                    </div>
                  </v-list-item>
                </v-col>
              </v-row>
            </v-expansion-panel-text>
          </v-expansion-panel>
        </v-expansion-panels>
      </v-col>
    </v-row>
    <v-row>
      <v-col cols="12">
        <leaderboard-claim :claim-id="claim?.entity_id"></leaderboard-claim>
      </v-col>
      <v-col cols="12">
        <v-expansion-panels v-model="buildings_collapsible">
          <v-expansion-panel value="buildings">
            <v-expansion-panel-title>
              <v-row>
                <v-col class="d-flex justify-center">
                  <h2 class="pl-md-3 pl-xl-0">Buildings ({{ buildings.length || 0 }})</h2>
                </v-col>
              </v-row>
            </v-expansion-panel-title>
            <v-expansion-panel-text>
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
                  <nuxt-link :to="{ name: 'buildings-id', params: { id: building.entity_id } }"
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
            </v-expansion-panel-text>
          </v-expansion-panel>
        </v-expansion-panels>
      </v-col>
    </v-row>
    <v-row>
      <v-col cols="12">
        <v-expansion-panels v-model="inventory_changelog_collapsible">
          <v-expansion-panel value="inventory_changelogs">
            <v-expansion-panel-title>
              <v-row>
                <v-col class="d-flex justify-center">
                  <h2 class="pl-md-3 pl-xl-0">Inventory Changes ({{ InventoryChangelogFetch?.length || 0 }})</h2>
                </v-col>
              </v-row>
            </v-expansion-panel-title>
            <v-expansion-panel-text>
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
            </v-expansion-panel-text>
          </v-expansion-panel>
        </v-expansion-panels>
      </v-col>
    </v-row>
  </v-container>
</template>
