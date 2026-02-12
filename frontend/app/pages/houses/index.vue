<script setup lang="ts">
import { watchDebounced } from "@vueuse/shared";
import type { HousesResponse } from "~/types/HousesResponse";

const page = ref(1);
const perPage = 24;

const search = ref<string>("");
const debouncedSearch = ref<string>("");
const showOnlyOnlineOwners = ref<boolean>(false);
const showOnlyEmptyHouses = ref<boolean | null>(null); // null = all, true = empty, false = occupied

const route = useRoute();
const router = useRouter();

if (route.query.owner) {
  debouncedSearch.value = route.query.owner as string;
  search.value = route.query.owner as string;
}

if (route.query.page) {
  page.value = parseInt(route.query.page as string);
}

if (route.query.online) {
  showOnlyOnlineOwners.value = route.query.online === "true";
}

if (route.query.empty) {
  showOnlyEmptyHouses.value = route.query.empty === "true";
} else if (route.query.occupied === "true") {
  showOnlyEmptyHouses.value = false;
}

const {
  data: housesResponse,
  pending,
  refresh,
} = await useLazyFetchMsPack<HousesResponse>(
  () => {
    return `/api/bitcraft/houses`;
  },
  {
    onRequest: ({ options }) => {
      options.query = options.query || {};

      if (search.value) {
        options.query.owner = search.value;
      }

      if (page.value) {
        options.query.page = page.value;
      }

      if (perPage) {
        options.query.per_page = perPage;
      }

      if (showOnlyOnlineOwners.value) {
        options.query.online = "true";
      }

      if (showOnlyEmptyHouses.value === true) {
        options.query.empty = "true";
      } else if (showOnlyEmptyHouses.value === false) {
        options.query.empty = "false";
      }

      if (Object.keys(options.query).length > 2) {
        const query = { ...options.query };
        delete query.per_page;
        router.push({ query });
      } else if (options.query.page <= 1) {
        router.push({});
      }
    },
  },
);

const currentHouses = computed(() => {
  return housesResponse.value?.houses ?? [];
});

const totalItems = computed(() => {
  if (housesResponse.value?.total) {
    return Number(housesResponse.value.total);
  }
  return 0;
});

const pageCount = computed(() => {
  return Math.ceil(totalItems.value / perPage);
});

const changePage = (value: number) => {
  page.value = value;
  router.push({
    query: {
      ...route.query,
      page: value,
    },
  });
  refresh();
};

watchDebounced(
  debouncedSearch,
  () => {
    if (search.value !== debouncedSearch.value) {
      page.value = 1;
    }
    search.value = debouncedSearch.value;
    refresh();
  },
  { debounce: 500 },
);

watch([showOnlyOnlineOwners, showOnlyEmptyHouses], () => {
  page.value = 1;
  refresh();
});

useSeoMeta({
  title: "Houses | BitCraft Hub",
  description: "Browse and search for player houses in BitCraft.",
});
</script>

<template>
  <v-container fluid>
    <v-row align="center">
      <v-col cols="12" md="6">
        <h1 class="text-h4 font-weight-black mb-1">Player Houses</h1>
        <div class="text-subtitle-1 text-grey mb-4" v-if="!pending || housesResponse">
          Showing {{ currentHouses.length }} of {{ totalItems }} results
        </div>
      </v-col>
      <v-col cols="12" md="6">
        <v-text-field
          v-model="debouncedSearch"
          label="Search by Owner (Username or Entity ID)"
          prepend-inner-icon="mdi-magnify"
          outlined
          dense
          clearable
          hide-details
          class="mb-4"
        ></v-text-field>
        <div class="d-flex flex-wrap gap-4">
          <v-checkbox
            v-model="showOnlyOnlineOwners"
            label="Online Owners"
            hide-details
            density="compact"
          ></v-checkbox>
          <v-btn-toggle v-model="showOnlyEmptyHouses" density="compact" color="primary" class="ml-auto" mandatory>
             <v-btn :value="null">All</v-btn>
             <v-btn :value="true">Empty</v-btn>
             <v-btn :value="false">Occupied</v-btn>
          </v-btn-toggle>
        </div>
      </v-col>
    </v-row>

    <v-row v-if="pending && !housesResponse">
      <v-col cols="12" class="text-center py-10">
        <v-progress-circular color="primary" indeterminate size="64"></v-progress-circular>
      </v-col>
    </v-row>

    <v-row v-else-if="currentHouses.length > 0">
      <v-col
        v-for="house in currentHouses"
        :key="house.entity_id.toString()"
        cols="12"
        sm="6"
        md="4"
        lg="3"
      >
        <bitcraft-house-card :house="house" />
      </v-col>
      
      <v-col cols="12" class="d-flex justify-center mt-6">
        <v-pagination
          v-model="page"
          :length="pageCount"
          :total-visible="7"
          @update:model-value="changePage"
        ></v-pagination>
      </v-col>
    </v-row>

    <v-row v-else-if="!pending">
      <v-col cols="12" class="text-center py-10">
        <v-icon icon="mdi-home-search-outline" size="64" color="grey"></v-icon>
        <div class="text-h6 text-grey mt-4">No houses found</div>
        <div v-if="search || showOnlyOnlineOwners || showOnlyEmptyHouses !== null" class="text-body-2 text-grey">
          Try adjusting your filters
        </div>
      </v-col>
    </v-row>
  </v-container>
</template>

<style scoped>
.gap-4 {
  gap: 16px;
}
</style>
