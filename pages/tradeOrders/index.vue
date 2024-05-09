<script setup lang="ts">
import { watchThrottled } from "@vueuse/shared";

const page = ref(1);
const perPage = 24;

const search = ref<string | null>("");

const route = useRoute();
const router = useRouter();

const tmpPage = (route.query.page as string) ?? null;

if (tmpPage) {
  page.value = parseInt(tmpPage);
}

const { data: tradeOrders, pending } = useFetch(() => {
  const url = new URLSearchParams();

  if (search.value) {
    url.append("search", search.value);
  }

  if (page.value) {
    url.append("page", page.value.toString());
  }

  if (perPage) {
    url.append("perPage", perPage.toString());
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

const currentTradeOrders = computed(() => {
  return tradeOrders.value?.trade_orders ?? [];
});

const length = computed(() => {
  if (tradeOrders.value?.total) {
    return Math.ceil( tradeOrders.value?.total / perPage)
  }

  return 0;
});

const theme = useTheme();

const computedClass = computed(() => {
  return {
    "bg-surface-light": theme.global.current.value.dark,
    "bg-grey-lighten-3": !theme.global.current.value.dark,
  };
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
    <v-col cols="12" md="6" lg="4" xl="3" xxl="2" v-for="tradeOrder in currentTradeOrders" :key="tradeOrder.entity_id">
    <v-card>
    <v-toolbar density="compact" color="transparent">
      <nuxt-link class="text-decoration-none text-high-emphasis font-weight-black" :to="{ name: 'buildings-id', params: { id: tradeOrder.building_entity_id } }"
        >{{ tradeOrder.entity_id }} : {{ tradeOrder.building_entity_id }}</nuxt-link>

    </v-toolbar>

    <v-card-text :class="computedClass">
        <v-list :class="computedClass">
          <v-list-item>
            <v-list-item-title>remaining_stock</v-list-item-title>
            <v-list-item-subtitle>{{ tradeOrder.remaining_stock }}</v-list-item-subtitle>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>offer_items</v-list-item-title>
            <v-list-item v-for="offer_item of tradeOrder.offer_items">
              <bitcraft-item :item="offer_item"></bitcraft-item>
            </v-list-item>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>offer_cargo</v-list-item-title>
            <v-list-item-subtitle>{{ tradeOrder.offer_cargo_id }} :: {{ tradeOrder.offer_cargo }}</v-list-item-subtitle>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>required_items</v-list-item-title>
            <v-list-item v-for="required_item of tradeOrder.required_items">
              <bitcraft-item :item="required_item"></bitcraft-item>
            </v-list-item>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>required_cargo</v-list-item-title>
            <v-list-item-subtitle>{{ tradeOrder.required_cargo_id }} ::  {{ tradeOrder.required_cargo }}</v-list-item-subtitle>
            </v-list-item>
          </v-list>
    </v-card-text>
  </v-card>
    </v-col>
  </v-row>
</template>

<style scoped>
</style>