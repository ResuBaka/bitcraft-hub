<script setup lang="ts">
import { watchThrottled } from "@vueuse/shared";

const page = ref(1);
const perPage = 24;

const tag = ref(null);
const tier = ref(null);
const search = ref<string | null>("");

const route = useRoute();
const router = useRouter();

search.value = (route.query.search as string) ?? "";
tag.value = (route.query.tag as string) ?? null;

const tmpTier = (route.query.tier as string) ?? null;
const tmpPage = (route.query.page as string) ?? null;

if (tmpTier) {
  tier.value = parseInt(tmpTier);
}
if (tmpPage) {
  page.value = parseInt(tmpPage);
}

const { data: items, pending } = useFetch(() => {
  const url = new URLSearchParams();

  if (search.value) {
    url.append("search", search.value);
  }

  if (tag.value) {
    url.append("tag", tag.value);
  }

  if (tier.value) {
    url.append("tier", tier.value.toString());
  }

  if (page.value) {
    url.append("page", page.value.toString());
  }

  if (perPage) {
    url.append("perPage", perPage.toString());
  }

  const querys = url.toString();

  if (querys) {
    return `/api/bitcraft/items?${querys}`;
  }

  return `/api/bitcraft/items`;
});

watchThrottled(
  () => [search.value, tag.value, tier.value, page.value],
  () => {
    const newQuery = {};

    if (search.value) {
      newQuery.search = search.value;
    }

    if (tag.value) {
      newQuery.tag = tag.value;
    }

    if (tier.value) {
      newQuery.tier = tier.value;
    }

    if (page.value) {
      newQuery.page = page.value;
    }

    router.push({ query: newQuery });
  },
  { throttle: 50 },
);

const currentItems = computed(() => {
  return items.value?.items ?? [];
});

const tags = computed(() => {
  return items.value?.tags ?? [];
});

const tiers = computed(() => {
  return items.value?.tiers ?? [];
});

const length = computed(() => {
  if (items.value?.total) {
    return Math.ceil(items.value?.total / perPage);
  }

  return 0;
});
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
          :items="tags"
          label="Tag"
          outlined
          dense
          clearable
      ></v-autocomplete>
    </v-col>
    <v-col>
      <v-select
          v-model="tier"
          :items="tiers"
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
  <v-row>
    <v-col cols="12" md="6" lg="4" xl="3" v-for="item in currentItems" :key="item.id">
      <bitcraft-card-item :item="item"></bitcraft-card-item>
    </v-col>
  </v-row>
  <v-row>
    <v-col cols="12">
      <v-pagination
          v-model="page"
          :length="length"
      ></v-pagination>
    </v-col>
  </v-row>
</template>

<style scoped>
</style>