<script setup lang="ts">
import { watchThrottled } from "@vueuse/shared";
const {
  public: { iconDomain },
} = useRuntimeConfig();

const page = ref(1);
const perPage = 30;

const search = ref<string | null>("");

const route = useRoute();
const router = useRouter();

const tmpPage = (route.query.page as string) ?? null;

if (tmpPage) {
  page.value = parseInt(tmpPage);
}
const tmpSearch = (route.query.search as string) ?? null;

if (tmpSearch) {
  search.value = tmpSearch;
}
const {
  public: { api },
} = useRuntimeConfig();
const { new_api } = useConfigStore();

const { data, pending, refresh } = await useLazyFetch(
  () => {
    if (new_api) {
      return `${api.base}/api/bitcraft/desc/buildings`;
    } else {
      return `/api/bitcraft/desc/buildings`;
    }
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
        if (new_api) {
          options.query.per_page = perPage;
        } else {
          options.query.perPage = perPage;
        }
      }

      if (Object.keys(options.query).length > 2) {
        const query = { ...options.query };
        if (new_api) {
          delete options.query.per_page;
        } else {
          delete options.query.perPage;
        }
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
  () => [search.value],
  (value, oldValue) => {
    if (value[0] !== oldValue[0]) {
      page.value = 1;
    }

    refresh();
  },
  { throttle: 50 },
);

const currentBuildings = computed(() => {
  return data.value?.buildings ?? [];
});

const length = computed(() => {
  return data.value?.total
    ? Math.ceil(data.value?.total / data.value?.perPage)
    : 0;
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
      <v-col cols="12" md="4" v-for="building in currentBuildings" :key="building.id">
        <bitcraft-building-card :building="building"></bitcraft-building-card>
      </v-col>
    </v-row>
  </v-container>
</template>
