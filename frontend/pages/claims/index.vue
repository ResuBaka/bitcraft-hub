<script setup lang="ts">
import { watchThrottled } from "@vueuse/shared";

const page = ref(1);
const perPage = 24;

const search = ref<string | null>("");

const route = useRoute();
const router = useRouter();

const showEmptySupplies = ref(false);

if (route.query.search) {
  search.value = route.query.search as string;
}

if (route.query.page) {
  page.value = parseInt(route.query.page);
}
const {
  public: { api },
} = useRuntimeConfig();

const {
  data: claims,
  pending,
  refresh,
} = await useLazyFetch(
  () => {
    return `${api.base}/api/bitcraft/claims`;
  },
  {
    onRequest: ({ options }) => {
      options.query = options.query || {};

      if (search.value) {
        options.query.search = search.value;
      }

      if (page.value) {
        options.query.page = page.value;
      }

      if (perPage) {
        options.query.per_page = perPage;
      }

      if (showEmptySupplies.value) {
        options.query.ses = showEmptySupplies.value.toString();
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

watchThrottled(
  () => [search.value, showEmptySupplies.value],
  (value, oldValue) => {
    if (value[0] !== oldValue[0] || value[1] !== oldValue[1]) {
      page.value = 1;
    }

    refresh();
  },
  { throttle: 50 },
);

const currentClaims = computed(() => {
  return claims.value?.claims ?? [];
});

const length = computed(() => {
  if (claims.value?.total) {
    return Math.ceil(claims.value?.total / perPage);
  }

  return 0;
});
</script>

<template>
  <v-container fluid>
    <v-row>
      <v-col>
        <v-text-field
            v-model="search"
            label="Search"
            outlined
            dense
            clearable
        ></v-text-field>
      </v-col>
      <v-col sm="3" md="2">
        <v-checkbox
            v-model="showEmptySupplies"
            label="Show Empty Supplies"
        ></v-checkbox>
      </v-col>
    </v-row>
    <v-row>
      <v-col>
        <v-pagination
            @update:model-value="changePage"
            v-model="page"
            :length="length"
        ></v-pagination>
        <v-progress-linear
            color="yellow-darken-2"
            indeterminate
            :active="pending"
        ></v-progress-linear>
      </v-col>
    </v-row>
    <v-row>
      <v-col cols="12" md="6" lg="4" xl="3" xxl="2" v-for="claim in currentClaims" :key="claim.entity_id">
        <bitcraft-card-claim :claim="claim"></bitcraft-card-claim>
      </v-col>
    </v-row>
  </v-container>
</template>
