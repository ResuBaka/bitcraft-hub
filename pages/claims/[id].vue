<script setup lang="ts">
import LeaderboardClaim from "~/components/Bitcraft/LeaderboardClaim.vue";

const page = ref(1);
const perPage = 30;

const search = ref<string | null>("");

const route = useRoute();
const router = useRouter();

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

const length = computed(() => {
  return Math.ceil((buidlingsFetch.value?.total || 0) / perPage) ?? 0;
});
</script>

<template>
  <v-container fluid>
    <v-card v-if="claim !== undefined">
      <v-toolbar color="transparent">
        <v-toolbar-title>
          {{ claim.name }} : {{ claim.entity_id }}
        </v-toolbar-title>
      </v-toolbar>
      <v-card-text>
        <v-row>
          <v-col cols="6" md="3">
            <v-list-item>
              <v-list-item-title>Owner</v-list-item-title>
              <v-list-item-subtitle>{{ claim.owner_player_entity_id }}</v-list-item-subtitle>
            </v-list-item>
          </v-col>
          <v-col cols="6" md="3">
            <v-list-item>
              <v-list-item-title>Supplies</v-list-item-title>
              <v-list-item-subtitle>{{ claim.supplies }}</v-list-item-subtitle>
            </v-list-item>
          </v-col>
          <v-col cols="6" md="3">
            <v-list-item>
              <v-list-item-title>Tiles</v-list-item-title>
              <v-list-item-subtitle>{{ claim.tiles }}</v-list-item-subtitle>
            </v-list-item>
          </v-col>
          <v-col cols="6" md="3">
            <v-list-item>
              <v-list-item-title>Location</v-list-item-title>
              <v-list-item-subtitle>{{ claim.location[Object.keys(claim.location)[0]][0] }} x
                {{ claim.location[Object.keys(claim.location)[0]][1] }}
              </v-list-item-subtitle>
            </v-list-item>
          </v-col>
          <v-col cols="12">
            <v-list-item>
              <v-list-item-title>Members</v-list-item-title>
              <v-row>
                <v-col cols="12" md="3" lg="2" v-for="member in claim.members" :key="member.user_name">
                  <v-list-item-subtitle>
                    <nuxt-link
                        class="text-decoration-none text-high-emphasis font-weight-black"
                        :to="{ name: 'players-id', params: { id: member.entity_id } }"
                    >
                      {{ member.user_name }}
                    </nuxt-link>
                  </v-list-item-subtitle>
                </v-col>
              </v-row>
            </v-list-item>
          </v-col>
        </v-row>
      </v-card-text>
    </v-card>
    <v-card class="mt-5">
      <v-card-title>Buildings</v-card-title>
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
            <v-pagination
                v-model="page"
                :length="length"
            ></v-pagination>
            <v-progress-linear
                color="yellow-darken-2"
                indeterminate
                :active="buildingsPending"
            ></v-progress-linear>
          </v-col>
        </v-row>
        <v-row>
          <v-col cols="12" md="4" lg="3" xl="2" v-for="building in buildings" :key="claim.entity_id">
            <a :href="'/buildings/' + building.entity_id">
              <v-list-item-subtitle v-if="building.nickname !== ''">{{ building.nickname }}</v-list-item-subtitle>
              <v-list-item-subtitle v-else>{{ building.entity_id }}</v-list-item-subtitle>
            </a>
          </v-col>
        </v-row>
      </v-card-text>
    </v-card>
    <v-card class="mt-5">
      <leaderboard-claim :claim-id="parseInt($route.params.id)"></leaderboard-claim>
    </v-card>
  </v-container>
</template>
