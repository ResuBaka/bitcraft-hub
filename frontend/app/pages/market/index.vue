<script setup lang="ts">
import { watchThrottled } from "@vueuse/shared";
import type { MarketOrdersResponse } from "~/types/MarketOrdersResponse";
import { registerWebsocketMessageHandler } from "~/composables/websocket";
import type { ItemsAndCargollResponse } from "~/types/ItemsAndCargollResponse";

const page = ref(1);

const search = ref();

const route = useRoute();

if (route.query.search) {
  search.value = route.query.search as string;
}

if (route.query.page) {
  page.value = parseInt(route.query.page);
}

const { data: itemsAndCargo } =
  await useLazyFetchMsPack<ItemsAndCargollResponse>(() => {
    return `/api/bitcraft/itemsAndCargo/all`;
  });

const { data: marketOrders } = await useLazyFetchMsPack<MarketOrdersResponse>(
  () => {
    return `/market`;
  },
  {
    deep: true,
  },
);

const selectValues = computed(() => {
  if (!itemsAndCargo.value) {
    return [];
  }

  let values = [];

  for (const item of Object.values(itemsAndCargo.value.cargo_desc)) {
    values.push({
      title: `${item.name} - ${item.rarity} - ${item.tier}`,
      value: {
        ...item,
        item_type: 1,
      },
    });
  }
  for (const item of Object.values(itemsAndCargo.value.item_desc)) {
    values.push({
      title: `${item.name} - ${item.rarity} - ${item.tier}`,
      value: {
        ...item,
        item_type: 0,
      },
    });
  }

  return values;
});

registerWebsocketMessageHandler(
  "InsertBuyOrder",
  ["insert_buy_order"],
  (message) => {
    if (!marketOrders.value) {
      return;
    }
    if (
      !marketOrders.value.buy_orders[`${message.item_type}:${message.item_id}`]
    ) {
      marketOrders.value.buy_orders[`${message.item_type}:${message.item_id}`] =
        [message];
    } else {
      let index = marketOrders.value.buy_orders[
        `${message.item_type}:${message.item_id}`
      ].findIndex((buy_order) => buy_order.entity_id === message.entity_id);

      if (index > -1) {
        marketOrders.value.buy_orders[
          `${message.item_type}:${message.item_id}`
        ][index] = message;
      } else {
        marketOrders.value.buy_orders[
          `${message.item_type}:${message.item_id}`
        ].push(message);
      }
    }
  },
);

registerWebsocketMessageHandler(
  "UpdateBuyOrder",
  ["update_buy_order"],
  (message) => {
    if (!marketOrders.value) {
      return;
    }
    if (
      !marketOrders.value.buy_orders[`${message.item_type}:${message.item_id}`]
    ) {
      marketOrders.value.buy_orders[`${message.item_type}:${message.item_id}`] =
        [message];
    } else {
      let index = marketOrders.value.buy_orders[
        `${message.item_type}:${message.item_id}`
      ].findIndex((buy_order) => buy_order.entity_id === message.entity_id);

      if (index > -1) {
        marketOrders.value.buy_orders[
          `${message.item_type}:${message.item_id}`
        ][index] = message;
      } else {
        marketOrders.value.buy_orders[
          `${message.item_type}:${message.item_id}`
        ].push(message);
      }
    }
  },
);

registerWebsocketMessageHandler(
  "RemoveBuyOrder",
  ["remove_buy_order"],
  (message) => {
    if (!marketOrders.value) {
      return;
    }

    if (
      !marketOrders.value.buy_orders[`${message.item_type}:${message.item_id}`]
    ) {
    } else {
      let index = marketOrders.value.buy_orders[
        `${message.item_type}:${message.item_id}`
      ].findIndex((buy_order) => buy_order.entity_id === message.entity_id);

      if (index > -1) {
        marketOrders.value.buy_orders[
          `${message.item_type}:${message.item_id}`
        ].splice(index, 1);
      }
    }
  },
);

registerWebsocketMessageHandler(
  "InsertSellOrder",
  ["insert_sell_order"],
  (message) => {
    if (!marketOrders.value) {
      return;
    }
    if (
      !marketOrders.value.sell_orders[`${message.item_type}:${message.item_id}`]
    ) {
      marketOrders.value.sell_orders[
        `${message.item_type}:${message.item_id}`
      ] = [message];
    } else {
      let index = marketOrders.value.sell_orders[
        `${message.item_type}:${message.item_id}`
      ].findIndex((buy_order) => buy_order.entity_id === message.entity_id);

      if (index > -1) {
        marketOrders.value.sell_orders[
          `${message.item_type}:${message.item_id}`
        ][index] = message;
      } else {
        marketOrders.value.sell_orders[
          `${message.item_type}:${message.item_id}`
        ].push(message);
      }
    }
  },
);

registerWebsocketMessageHandler(
  "UpdateSellOrder",
  ["update_sell_order"],
  (message) => {
    if (!marketOrders.value) {
      return;
    }
    if (
      !marketOrders.value.sell_orders[`${message.item_type}:${message.item_id}`]
    ) {
      marketOrders.value.sell_orders[
        `${message.item_type}:${message.item_id}`
      ] = [message];
    } else {
      let index = marketOrders.value.sell_orders[
        `${message.item_type}:${message.item_id}`
      ].findIndex((buy_order) => buy_order.entity_id === message.entity_id);

      if (index > -1) {
        marketOrders.value.sell_orders[
          `${message.item_type}:${message.item_id}`
        ][index] = message;
      } else {
        marketOrders.value.sell_orders[
          `${message.item_type}:${message.item_id}`
        ].push(message);
      }
    }
  },
);

registerWebsocketMessageHandler(
  "RemoveSellOrder",
  ["remove_sell_order"],
  (message) => {
    if (!marketOrders.value) {
      return;
    }
    if (
      !marketOrders.value.sell_orders[`${message.item_type}:${message.item_id}`]
    ) {
    } else {
      let index = marketOrders.value.sell_orders[
        `${message.item_type}:${message.item_id}`
      ].findIndex((buy_order) => buy_order.entity_id === message.entity_id);

      if (index > -1) {
        marketOrders.value.sell_orders[
          `${message.item_type}:${message.item_id}`
        ].splice(index, 1);
      }
    }
  },
);

watchThrottled(
  () => [search.value],
  (value, oldValue) => {
    // router.push({ query: { search: value } });
  },
  { throttle: 50 },
);

const sellOrders = computed(() => {
  if (!marketOrders.value) {
    return [];
  }

  if (!search.value) {
    return [];
  }

  let orders = [];

  for (const selected of Object.values(search.value)) {
    console.log(
      "sell_orders selected",
      selected,
      marketOrders.value.sell_orders[`${selected.item_type}:${selected.id}`],
    );
    if (
      marketOrders.value.sell_orders[`${selected.item_type}:${selected.id}`]
    ) {
      orders.push(
        ...marketOrders.value.sell_orders[
          `${selected.item_type}:${selected.id}`
        ],
      );
    }
  }

  return orders;
});

const buyOrders = computed(() => {
  if (!marketOrders.value) {
    return [];
  }

  if (!search.value) {
    return [];
  }

  let orders = [];

  for (const selected of search.value) {
    console.log(
      "selected",
      selected,
      marketOrders.value.buy_orders[`${selected.item_type}:${selected.id}`],
    );
    if (marketOrders.value.buy_orders[`${selected.item_type}:${selected.id}`]) {
      orders.push(
        ...marketOrders.value.buy_orders[
          `${selected.item_type}:${selected.id}`
        ],
      );
    }
  }

  return orders;
});

const sellHeaders = [
  {
    align: "start",
    key: "item_id",
    sortable: false,
    title: "Item",
  },
  { key: "quantity", title: "Amount" },
  { key: "price_threshold", title: "Price" },
  // { key: "owner_entity_id", title: "Owner" },
  { key: "claim_entity_id", title: "Claim" },
  { key: "region", title: "Region" },
];

const buyHeaders = [
  {
    align: "start",
    key: "item_id",
    sortable: false,
    title: "Item",
  },

  { key: "quantity", title: "Amount" },
  { key: "price_threshold", title: "Price" },
  // { key: "stored_coins", title: "Coins" },
  // { key: "owner_entity_id", title: "Owner" },
  { key: "claim_entity_id", title: "Claim" },
  { key: "region", title: "Region" },
];
const tree = ref([]);
const types = ref([]);
const items = ref([]);

watch(itemsAndCargo, (val) => {
  let topLevel = [];
  let a = {};

  for (const item of Object.values(val.item_desc)) {
    if (!topLevel.includes(item.tag)) topLevel.push(item.tag);

    if (a[item.tag]) {
      a[item.tag].push({
        ...item,
        cal_id: `${0}_${item.id}`,
      });
    } else {
      a[item.tag] = [
        {
          ...item,
          cal_id: `${0}_${item.id}`,
        },
      ];
    }
  }

  for (const item of Object.values(val.cargo_desc)) {
    if (!topLevel.includes(item.tag)) topLevel.push(item.tag);

    if (a[item.tag]) {
      a[item.tag].push({
        ...item,
        cal_id: `${0}_${item.id}`,
      });
    } else {
      a[item.tag] = [
        {
          ...item,
          cal_id: `${0}_${item.id}`,
        },
      ];
    }
  }

  types.value = topLevel.sort();

  const children = types.value.map((type) => ({
    cal_id: type,
    name: type,
    children: a[type],
  }));
  items.value = children;
});
</script>

<template>
  <v-container fluid>
    <v-row>
      <v-col>
        <v-autocomplete
            v-model="search"
            :items="selectValues"
            multiple
            label="Search"
            outlined
            dense
            clearable
        ></v-autocomplete>
      </v-col>
    </v-row>
    <v-row>
      <v-col cols="2">
        <v-treeview
            v-model:selected="tree"
            :items="items"
            class="flex-1-0"
            false-icon="mdi-bookmark-outline"
            indeterminate-icon="mdi-bookmark-minus"
            item-title="name"
            item-value="cal_id"
            select-strategy="classic"
            true-icon="mdi-bookmark"
            return-object
            selectable
        ></v-treeview>
      </v-col>
      <v-col>
        <v-card
            title="Sell Orders"
            flat
        >
          <v-data-table
              :headers="sellHeaders"
              :items="sellOrders"
              :sort-by="[{ key: 'price_threshold', order: 'asc' },]"
          ></v-data-table>
        </v-card>
      </v-col>
      <v-col>
        <v-card
            title="Buy Orders"
            flat
        >
          <v-data-table
              :headers="buyHeaders"
              :items="buyOrders"
              :sort-by="[{ key: 'price_threshold', order: 'desc' },]"
          ></v-data-table>
        </v-card>
      </v-col>
    </v-row>
  </v-container>
</template>
