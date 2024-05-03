<script setup lang="ts">
import { watchThrottled } from "@vueuse/shared";

const page = ref(1);
const perPage = 30;

const search = ref<string | null>("");

const route = useRoute();
const router = useRouter();

const tmpPage = (route.query.page as string) ?? null;

if (tmpPage) {
  page.value = parseInt(tmpPage);
}

const { data: claims, pending } = useFetch(() => {
  const url = new URLSearchParams();

  if (search.value) {
    url.append("search", search.value);
  }

  if (page.value) {
    url.append("page", page.value.toString());
  }

  const querys = url.toString();

  if (querys) {
    return `/api/bitcraft/tradeOrders?${querys}`;
  }

  return `/api/bitcraft/tradeOrders`;
});

watchThrottled(
  () => [search.value, page.value],
  () => {
    const newQuery = {};

    if (search.value) {
      newQuery.search = search.value;
    }

    if (page.value) {
      newQuery.page = page.value;
    }

    router.push({ query: newQuery });
  },
  { throttle: 50 },
);

const currentClaims = computed(() => {
  return claims.value?.trade_orders ?? [];
});

const length = computed(() => {
  return Math.ceil(claims.value?.total / perPage) ?? 0;
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
  </v-row>
  <v-row>
    <v-col>
      <v-pagination
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
    <v-col cols="12" md="4" v-for="claim in currentClaims" :key="claim.entity_id">
      <v-card  v-if="claim !== undefined">
    <v-toolbar color="transparent">
      <v-toolbar-title>
        {{ claim.entity_id }}</v-toolbar-title>

    </v-toolbar>

    <v-card-text>
        <v-list>
          <v-list>
          <v-list-item>
            <v-list-item-title>remaining_stock</v-list-item-title>
            <v-list-item-subtitle>{{ claim.remaining_stock }}</v-list-item-subtitle>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>offer_items</v-list-item-title>
            <v-list-item v-for="offer_item of claim.offer_items">
              <bitcraft-item :item="offer_item"></bitcraft-item>
            </v-list-item>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>offer_cargo</v-list-item-title>
            <v-list-item-subtitle>{{ claim.offer_cargo_id }} :: {{ claim.offer_cargo }}</v-list-item-subtitle>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>required_items</v-list-item-title>
            <v-list-item v-for="required_item of claim.required_items">
              <bitcraft-item :item="required_item"></bitcraft-item>
            </v-list-item>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>required_cargo</v-list-item-title>
            <v-list-item-subtitle>{{ claim.required_cargo_id }} ::  {{ claim.required_cargo }}</v-list-item-subtitle>
            </v-list-item>
              </v-list>
              </v-list>
    </v-card-text>
  </v-card>
    </v-col>
  </v-row>
</template>

<style scoped>
</style>