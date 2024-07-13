<script setup lang="ts">
import LeaderboardClaim from "~/components/Bitcraft/LeaderboardClaim.vue";
import CardItem from "~/components/Bitcraft/CardItem.vue";
import { iconAssetUrlNameRandom } from "~/composables/iconAssetName";
const {
  public: { iconDomain },
} = useRuntimeConfig();

const page = ref(1);
const perPage = 30;

const search = ref<string | null>("");

const route = useRoute();
const router = useRouter();

const rarity = ref<string | null>(null);
const tier = ref<number | null>(null);

const tmpPage = (route.query.page as string) ?? null;

if (tmpPage) {
  page.value = parseInt(tmpPage);
}

const { data: claimFetch, pending: claimPnding } = useFetch(() => {
  return `/api/bitcraft/claims/${route.params.id}`;
});
const { data: buidlingsFetch, pending: buildingsPending } = useFetch(() => {
  return `/api/bitcraft/buildings?claim_entity_id=${route.params.id}&with_inventory=true&page=${page.value}`;
});

const claim = computed(() => {
  return claimFetch.value ?? undefined;
});
const buildings = computed(() => {
  return buidlingsFetch.value?.buildings ?? [];
});

const inventorySearch = ref<string | null>("");

const inventorys = computed(() => {
  if (!claimFetch.value?.inventorys.length) {
    return [];
  }

  return claimFetch.value?.inventorys.filter(
    (inventory) =>
      (!rarity.value ||
        parseInt(Object.keys(inventory.item.rarity)[0]) === rarity.value) &&
      (!tier.value || inventory.item.tier === tier.value) &&
      (!inventorySearch.value ||
        inventory.item.name
          .toLowerCase()
          .includes(inventorySearch.value.toLowerCase())),
  );
});

const tierColor = function (tier: number) {
  const colors = {
    1: "grey",
    2: "green",
    3: "blue",
    4: "purple",
    5: "yellow",
    6: "pink",
  };

  return colors[tier] ?? "";
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
                  <v-list-item-subtitle>{{ claim.tiles }}</v-list-item-subtitle>
                </v-list-item>
              </v-col>
              <v-col cols="6" md="2" lg="12">
                <v-list-item>
                  <v-list-item-title>Location</v-list-item-title>
                  <v-list-item-subtitle>
                    N: {{ Math.ceil(claim.location[Object.keys(claim.location)[0]][1] / 3)  }}, E: {{ Math.ceil(claim.location[Object.keys(claim.location)[0]][0] / 3) }}
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
            </v-row>
          </v-card-text>
        </v-card>
      </v-col>
      <v-col cols="12" lg="10">
        <v-card height="100%" title="Members">
          <v-card-text>
              <v-list-item>
                <v-list-item-title>Members ({{ claim?.members?.length || 0 }})</v-list-item-title>
                <v-row>
                  <v-col cols="6" md="3" xl="2" v-for="member in sortedUsersByPermissionLevel" :key="member.user_name">
                    <nuxt-link
                        class="text-decoration-none text-high-emphasis font-weight-black"
                        :to="{ name: 'players-id', params: { id: member.entity_id } }"
                    >
                      {{ member.user_name }}
                      {{ member.co_owner_permission ? "🏰" : "" }}
                      {{ member.officer_permission ? "🗡️" : "" }}
                      {{ member.build_permission ? "🔨" : "" }}
                      {{ member.inventory_permission ? "📦" : "" }}
                    </nuxt-link>
                  </v-col>
                </v-row>
              </v-list-item>
          </v-card-text>
        </v-card>
      </v-col>
    </v-row>
    <v-row>
      <v-col cols="12">
      <v-card class="mt-5">
        <v-card-item>
          <v-card-title>Items ({{ inventorys.length || 0 }})</v-card-title>
        </v-card-item>
        <v-card-text>
          <v-row>
            <v-col>
              <v-text-field
                  v-model="inventorySearch"
                  label="Search"
                  outlined
                  dense
                  clearable
              ></v-text-field>
            </v-col>
            <v-col>
              <v-autocomplete
                  v-model="tier"
                  :items="Array.from(new Set(claimFetch?.inventorys.map((inventory) => inventory.item.tier) || []))"
                  label="Tier"
                  outlined
                  dense
                  clearable
              ></v-autocomplete>
            </v-col>
            <v-col>
              <v-select
                  v-model="rarity"
                  :items="Array.from(new Set(claimFetch?.inventorys.map((inventory) => parseInt(Object.keys(inventory.item.rarity)[0])) || []))"
                  label="Rarity"
                  outlined
                  dense
                  clearable
              ></v-select>
            </v-col>
          </v-row>
          <v-row>
            <v-col cols="12" md="4" lg="3" xl="2" v-for="inventory in inventorys" :key="inventory.item_id">
                <v-list-item>
                  <template #prepend v-if="iconDomain">
                    <v-avatar :image="iconAssetUrlNameRandom(inventory.item.icon_asset_name).url" size="50"></v-avatar>
                  </template>
                  <div :class="`text-${tierColor(inventory.item.tier)}`">
                    {{ inventory.item.name }}:
                    <strong>{{ inventory.quantity }}</strong>
                  </div>
                </v-list-item>
            </v-col>
          </v-row>
        </v-card-text>
      </v-card>
      </v-col>
    </v-row>
    <v-row>
      <v-col cols="12">
        <v-card class="mt-5">
          <leaderboard-claim :claim-id="parseInt($route.params.id)"></leaderboard-claim>
        </v-card>
      </v-col>
      <v-col cols="12">
      <v-card class="mt-5">
        <v-card-item>
          <v-card-title>Buildings</v-card-title>
        </v-card-item>
        <v-card-text>
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
            <v-col cols="12" md="4" lg="3" xl="2" v-for="building in buildings" :key="claim.entity_id">
              <nuxt-link :to="{ name: 'buildings-id', params: { id: building.entity_id } }" class="text-high-emphasis font-weight-black">
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
        </v-card-text>
      </v-card>
      </v-col>
    </v-row>
  </v-container>
</template>
