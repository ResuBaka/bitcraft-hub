<script setup lang="ts">
import LeaderboardClaim from "~/components/Bitcraft/LeaderboardClaim.vue";
import CardItem from "~/components/Bitcraft/CardItem.vue";
import { iconAssetUrlNameRandom } from "~/composables/iconAssetName";

const {
  public: { iconDomain },
} = useRuntimeConfig();

const page = ref(1);
const perPage = 500;

const building_items_collapsible = ref([]);
const player_items_collapsible = ref([]);
const player_offline_items_collapsible = ref([]);
const buildings_collapsible = ref([]);

const search = ref<string | null>("");

const route = useRoute();
const router = useRouter();

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

const { data: claimFetch, pending: claimPnding } = useFetch(() => {
  return `${api.base}/api/bitcraft/claims/${route.params.id}`;
});

const { data: buidlingsFetch, pending: buildingsPending } = useFetch(() => {
  return `${api.base}/api/bitcraft/buildings?claim_entity_id=${route.params.id}&with_inventory=true&page=${page.value}&per_page=${perPage}`;
});

const claim = computed(() => {
  return claimFetch.value ?? undefined;
});
const buildings = computed(() => {
  if (!buidlingsFetch.value?.buildings.length) {
    return [];
  }

  return buidlingsFetch.value?.buildings.filter((building) => {
    let name_to_check = building.nickname ?? building.building_name;
    return (
      !search.value ||
      name_to_check.toLowerCase().includes(search.value.toLowerCase())
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

const length = computed(() => {
  return Math.ceil((buidlingsFetch.value?.total || 0) / perPage) ?? 0;
});

const claimOwner = computed(() => {
  if (claim.value === undefined) {
    return "";
  }

  return (
    claim.value.members.find(
      (member) => member.entity_id === claim.value.owner_player_entity_id,
    )?.user_name ?? ""
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

let memberSearch = ref<string | null>(null);

const membersForTable = computed(() => {
  if (!claim.value?.members) {
    return [];
  }
  return claim.value.members
    .filter((member) => {
      if (memberSearch.value) {
        return member.user_name
          .toLowerCase()
          .includes(memberSearch.value.toLowerCase());
      }

      return true;
    })
    .map((member) => {
      let permissions = 0;
      if (member.co_owner_permission) {
        permissions += 8;
      }
      if (member.officer_permission) {
        permissions += 4;
      }
      if (member.build_permission) {
        permissions += 2;
      }
      if (member.inventory_permission) {
        permissions += 1;
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
                  <v-list-item-subtitle>{{ claimOwner }}</v-list-item-subtitle>
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
                  <v-list-item-title>Current xp for minting</v-list-item-title>
                  <v-list-item-subtitle>{{ claim.xp_gained_since_last_coin_minting }}</v-list-item-subtitle>
                </v-list-item>
              </v-col>
              <v-col cols="6" md="2" lg="12">
                <v-list-item>
                  <v-list-item-title>Location</v-list-item-title>
                  <v-list-item-subtitle>
                    N: {{ Math.ceil(claim.location[Object.keys(claim.location)[0]][1] / 3) }}, E:
                    {{ Math.ceil(claim.location[Object.keys(claim.location)[0]][0] / 3) }}
                  </v-list-item-subtitle>
                </v-list-item>
              </v-col>
              <v-col cols="6" md="2" lg="12">
                <v-list-item>
                  <v-list-item-title>Buildings</v-list-item-title>
                  <v-list-item-subtitle>
                    {{ buidlingsFetch?.total || 0 }}
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
                  <v-list-item-title>Running Upgrade</v-list-item-title>
                  <v-list-item-subtitle>
                    <strong>{{ claimFetch?.running_upgrade.description }}</strong> is going to be finished at: <strong>{{ nDate.format(new Date((claimFetch?.running_upgrade.research_time * 1000) + (claimFetch?.running_upgrade_started / 1000))) }}</strong>
                  </v-list-item-subtitle>
                </v-list-item>
              </v-col>
              <v-col cols="6" md="2" lg="12">
                <v-list-item>
                  <v-list-item-title>Time played</v-list-item-title>
                  <v-list-item-subtitle>
                    {{ secondsToDaysMinutesSecondsFormat(claimFetch?.time_played) }}
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
            Members ({{claim?.members?.length || 0}})

            <v-spacer></v-spacer>

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
                density="compact"
                :sort-by="[{ key: 'permissions', order: 'desc' }]"
                :headers="[
                {
                  title: 'User',
                  key: 'user_name',
                  cellProps: {
                    class: 'font-weight-black'
                  }
                },
                 {
                  title: 'Co-Owner',
                  key: 'permissions',
                },
                  {
                    title: 'Carpentry',
                    key: 'skills_ranks.Carpentry',
                    sort: sortMembersLevelRaw
                  },
                  {
                    title: 'Cooking',
                    key: 'skills_ranks.Cooking',
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
              <template #item.permissions="{ item }">
                {{ item.co_owner_permission ? "🏰" : "" }}
                {{ item.officer_permission ? "🗡️" : "" }}
                {{ item.build_permission ? "🔨" : "" }}
                {{ item.inventory_permission ? "📦" : "" }}
              </template>
              <template #item.skills_ranks.Carpentry="{ value }">
                <v-chip class="font-weight-black" :color="levelToColor(value.level)">{{ value.level }}</v-chip>
              </template>
              <template #item.skills_ranks.Cooking="{ value }">
                <v-chip class="font-weight-black" :color="levelToColor(value.level)">{{ value.level }}</v-chip>
              </template>
              <template #item.skills_ranks.Farming="{ value }">
                <v-chip class="font-weight-black" :color="levelToColor(value.level)">{{ value.level }}</v-chip>
              </template>
              <template #item.skills_ranks.Fishing="{ value }">
                <v-chip class="font-weight-black" :color="levelToColor(value.level)">{{ value.level }}</v-chip>
              </template>
              <template #item.skills_ranks.Foraging="{ value }">
                <v-chip class="font-weight-black" :color="levelToColor(value.level)">{{ value.level }}</v-chip>
              </template>
              <template #item.skills_ranks.Forestry="{ value }">
                <v-chip class="font-weight-black" :color="levelToColor(value.level)">{{ value.level }}</v-chip>
              </template>
              <template #item.skills_ranks.Hunting="{ value }">
                <v-chip class="font-weight-black" :color="levelToColor(value.level)">{{ value.level }}</v-chip>
              </template>
              <template #item.skills_ranks.Leatherworking="{ value }">
                <v-chip class="font-weight-black" :color="levelToColor(value.level)">{{ value.level }}</v-chip>
              </template>
              <template #item.skills_ranks.Masonry="{ value }">
                <v-chip class="font-weight-black" :color="levelToColor(value.level)">{{ value.level }}</v-chip>
              </template>
              <template #item.skills_ranks.Mining="{ value }">
                <v-chip class="font-weight-black" :color="levelToColor(value.level)">{{ value.level }}</v-chip>
              </template>
              <template #item.skills_ranks.Scholar="{ value }">
                <v-chip class="font-weight-black" :color="levelToColor(value.level)">{{ value.level }}</v-chip>
              </template>
              <template #item.skills_ranks.Smithing="{ value }">
                <v-chip class="font-weight-black" :color="levelToColor(value.level)">{{ value.level }}</v-chip>
              </template>
              <template #item.skills_ranks.Tailoring="{ value }">
                <v-chip class="font-weight-black" :color="levelToColor(value.level)">{{ value.level }}</v-chip>
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
        <leaderboard-claim :claim-id="parseInt($route.params.id)"></leaderboard-claim>
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
                      <template v-if="building.nickname !== ''">{{ building.nickname }}</template>
                      <template v-else>{{ building.building_name }}</template>
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
  </v-container>
</template>
