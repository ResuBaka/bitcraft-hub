<script setup lang="ts">
import { watchThrottled } from "@vueuse/shared";
import type { TradeOrdersResponse } from "~/types/TradeOrdersResponse";

const page = ref(1);
const perPage = 24;

const search = ref<string | null>("");

const route = useRoute();
const router = useRouter();

if (route.query.search) {
  search.value = route.query.search as string;
}

if (route.query.page) {
  page.value = parseInt(route.query.page);
}

const {
  data: tradeOrders,
  pending,
  refresh,
} = await useLazyFetchMsPack<TradeOrdersResponse>(
  () => {
    return `/api/bitcraft/trade_orders/get_trade_orders`;
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

// watchThrottled(
//   () => [search.value],
//   (value, oldValue) => {
//     if (value[0] !== oldValue[0]) {
//       page.value = 1;
//     }
//
//     refresh();
//   },
//   { throttle: 50 },
// );

const currentTradeOrders = computed(() => {
  if (!tradeOrders.value) {
    return [];
  }

  if (!search.value) {
    return tradeOrders.value.trade_orders;
  }

  let internalSearch = Number(search.value);

  return tradeOrders.value.trade_orders.filter((tradeOrder) => {
    if (tradeOrder.required_items.find((ri) => ri.item_id == internalSearch)) {
      return true;
    }

    if (tradeOrder.offer_items.find((ri) => ri.item_id == internalSearch)) {
      return true;
    }

    if (tradeOrder.offer_cargo_id.includes(internalSearch)) {
      return true;
    }

    if (tradeOrder.required_cargo_id.includes(internalSearch)) {
      return true;
    }

    return false;
  });
});

const length = computed(() => {
  if (tradeOrders.value?.total) {
    return Math.ceil(tradeOrders.value?.total / perPage);
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
      <v-data-iterator :items="currentTradeOrders" :items-per-page="50" class="w-100">
        <template v-slot:default="{ items }">
          <v-container class="ma-0" fluid>
          <v-row >
            <template
                v-for="(tradeOrder, i) in items"
                :key="i"
            >
              <v-col>
              <v-card>
                <v-card-item>
                  <nuxt-link class="text-decoration-none text-high-emphasis font-weight-black"
                             :to="{ name: 'buildings-id', params: { id: tradeOrder.raw.shop_entity_id } }"
                  >{{ tradeOrder.raw.shop_name }} ({{ tradeOrder.raw.shop_entity_id }})
                  </nuxt-link>
                </v-card-item>
                <v-card-text class="h-100" :class="computedClass">
                  <v-table :class="computedClass" density="compact">
                    <tbody>
                    <tr style='text-align: right'>
                      <th>Remaining stock:</th>
                      <td>{{ tradeOrder.raw.remaining_stock }}</td>
                    </tr>
                    </tbody>
                  </v-table>
                  <v-toolbar density="compact" class="mt-2" color="secondary-darken-1" title="Offer Item/Cargo"></v-toolbar>
                  <v-table density="compact">
                    <thead>
                    <tr>
                      <th>Icon</th>
                      <th>Name</th>
                      <th>Quantity</th>
                      <th>Type</th>
                    </tr>
                    </thead>
                    <tbody>
                    <tr v-for="offer_item of tradeOrder.raw.offer_items">
                      <bitcraft-item :item="offer_item"></bitcraft-item>
                    </tr>
                    <tr v-for="offer_item of tradeOrder.raw.offer_cargo">
                      <td>
                        <v-img :src="iconAssetUrlName(offer_item.item.icon_asset_name).url" height="50" width="50"></v-img>
                      </td>
                      <td>{{ offer_item.name }}</td>
                      <td>1</td>
                      <td>Cargo</td>
                    </tr>
                    </tbody>
                  </v-table>

                  <v-toolbar density="compact" class="mt-2" color="secondary-darken-1" title="Require Item/Cargo"></v-toolbar>
                  <v-table density="compact">
                    <thead>
                    <tr>
                      <th>Icon</th>
                      <th>Name</th>
                      <th>Quantity</th>
                      <th>Type</th>
                    </tr>
                    </thead>
                    <tbody>
                    <tr v-for="requiredItem of tradeOrder.raw.required_items">
                      <bitcraft-item :item="requiredItem"></bitcraft-item>
                    </tr>
                    <tr v-for="requiredCargo of tradeOrder.raw.required_cargo">
                      <td>
                        <v-img :src="iconAssetUrlName(requiredCargo.item.icon_asset_name).url" height="50" width="50"></v-img>
                      </td>
                      <td>{{ requiredCargo.name }}</td>
                      <td>1</td>
                      <td>Cargo</td>
                    </tr>
                    </tbody>
                  </v-table>
                </v-card-text>
              </v-card>
              </v-col>
            </template>
          </v-row>
          </v-container>
        </template>
        <template v-slot:footer="{ page, pageCount, prevPage, nextPage }">
          <div class="d-flex align-center justify-center pa-4">
            <v-btn
                :disabled="page === 1"
                density="comfortable"
                icon="mdi-arrow-left"
                variant="tonal"
                rounded
                @click="prevPage"
            ></v-btn>

            <div class="mx-2 text-caption">
              Page {{ page }} of {{ pageCount }}
            </div>

            <v-btn
                :disabled="page >= pageCount"
                density="comfortable"
                icon="mdi-arrow-right"
                variant="tonal"
                rounded
                @click="nextPage"
            ></v-btn>
          </div>
        </template>
      </v-data-iterator>
    </v-row>
  </v-container>
</template>
