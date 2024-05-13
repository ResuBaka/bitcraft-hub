<script setup lang="ts">
import { watchThrottled } from "@vueuse/shared";

const page = ref(1);
const perPage = 24;

const tag = ref<string | null>(null);
const tier = ref<number | null>(null);
const search = ref<string | null>("");

const route = useRoute();
const router = useRouter();

search.value = (route.query.search as string) ?? "";
tag.value = (route.query.tag as string) ?? null;

if (route.query.tier) {
  tier.value = parseInt(route.query.tier);
}
if (route.query.page) {
  page.value = parseInt(route.query.page);
}

const { data, pending, refresh } = await useLazyFetch(`/api/bitcraft/items`, {
  onRequest: ({ options }) => {
    options.query = options.query || {};

    if (search.value) {
      options.query.search = search.value;
    }

    if (page.value) {
      options.query.page = page.value;
    }

    if (tag.value) {
      options.query.tag = tag.value;
    }

    if (tier.value) {
      options.query.tier = tier.value;
    }

    if (perPage) {
      options.query.perPage = perPage;
    }

    if (Object.keys(options.query).length > 2) {
      const query = { ...options.query };
      delete query.perPage;
      router.push({ query });
    } else if (options.query.page <= 1) {
      router.push({});
    }
  },
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

watchThrottled(
  () => [search.value, tag.value, tier.value],
  (value, oldValue) => {
    if (
      value[0] !== oldValue[0] ||
      value[1] !== oldValue[1] ||
      value[2] !== oldValue[2]
    ) {
      page.value = 1;
    }

    refresh();
  },
  { throttle: 50 },
);
</script>

<template>
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
    <v-col>
      <v-autocomplete
          v-model="tag"
          :items="data?.tags || []"
          label="Tag"
          outlined
          dense
          clearable
      ></v-autocomplete>
    </v-col>
    <v-col>
      <v-select
          v-model="tier"
          :items="data?.tiers || []"
          label="Tier"
          outlined
          dense
          clearable
      ></v-select>
    </v-col>
  </v-row>

  <v-row>
    <v-col>
      <v-progress-linear
          color="yellow-darken-2"
          indeterminate
          :active="pending"
      ></v-progress-linear>
    </v-col>
  </v-row>
  <template v-if="data">
    <v-row>
      <v-col cols="12" md="6" lg="4" xl="3" v-for="item in data.items" :key="item.id">
        <bitcraft-card-item :item="item"></bitcraft-card-item>
      </v-col>
    </v-row>
    <v-row>
      <v-col cols="12">
        <v-pagination
            @update:model-value="changePage"
            v-model="page"
            :length="data?.pages || 0"
        ></v-pagination>
      </v-col>
    </v-row>
  </template>
  <template v-else>
    <v-card>
      <v-card-text>
        No items found
      </v-card-text>
    </v-card>
  </template>
</template>

<style scoped>
</style>