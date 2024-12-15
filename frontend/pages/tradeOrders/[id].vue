<script setup lang="ts">
const page = ref(1);
const perPage = 30;

const search = ref<string | null>("");
const route = useRoute();
const tmpPage = (route.query.page as string) ?? null;

if (tmpPage) {
  page.value = parseInt(tmpPage);
}

const {
  public: { api },
} = useRuntimeConfig();

const { data: claimFetch, pending: claimPnding } = useFetch(() => {
  return `${api.base}/api/bitcraft/claims/${route.params.id}?per_Page=${perPage.value}`;
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
          >{{ claim.name }} : {{ claim.entity_id }}
        </v-toolbar-title>

      </v-toolbar>

      <v-card-text>
        <v-list>
          <v-list>
            <v-list-item>
              <v-list-item-title>Owner</v-list-item-title>
              <v-list-item-subtitle>{{ claim.owner_player_entity_id }}</v-list-item-subtitle>
            </v-list-item>
            <v-list-item>
              <v-list-item-title>Supplies</v-list-item-title>
              <v-list-item-subtitle>{{ claim.supplies }}</v-list-item-subtitle>
            </v-list-item>
            <v-list-item>
              <v-list-item-title>Tiles</v-list-item-title>
              <v-list-item-subtitle>{{ claim.tiles }}</v-list-item-subtitle>
            </v-list-item>
            <v-list-item>
              <v-list-item-title>Location</v-list-item-title>
              <v-list-item-subtitle>{{ claim.location }}</v-list-item-subtitle>
            </v-list-item>
            <v-list-item-title>Members</v-list-item-title>
            <v-row>
              <v-col cols="12" md="3" v-for="member in claim.members" :key="member.user_name">
                <v-list-item-subtitle>{{ member.user_name }}</v-list-item-subtitle>
              </v-col>
            </v-row>
            <v-list-item>
              <v-list-item-title>Buildings</v-list-item-title>
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
                <v-col cols="12" md="4" v-for="building in buildings" :key="claim.entity_id">
                  <a :href="'/buildings/' + building.entity_id">
                    <v-list-item-subtitle v-if="building.nickname !== ''">{{ building.nickname }}</v-list-item-subtitle>
                    <v-list-item-subtitle v-else>{{ building.entity_id }}</v-list-item-subtitle>
                  </a>
                </v-col>
              </v-row>
            </v-list-item>
          </v-list>
        </v-list>
      </v-card-text>
    </v-card>
  </v-container>
</template>
