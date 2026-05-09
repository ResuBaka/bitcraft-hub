<script setup lang="ts">
import type { TableColumn, TreeItem as NuxtTreeItem } from "@nuxt/ui";
import { watchThrottled } from "@vueuse/shared";
import { registerWebsocketMessageHandler } from "~/composables/websocket";
import type { AuctionListingState } from "~/types/AuctionListingState";
import type { ItemOption } from "~/types/ItemOption";
import type { MarketItemCargoDescResponse } from "~/types/MarketItemCargoDescResponse";
import type { MarketOrderStats } from "~/types/MarketOrderStats";
import type { MarketOrderStatsResponse } from "~/types/MarketOrderStatsResponse";
import type { MarketOrdersResponse } from "~/types/MarketOrdersResponse";
import { raritySort, rarityToTextClass, tierToTextClass, useDelayedPending } from "~/utils";

type PreparedItemOption = ItemOption & { searchLabel: string };
type ItemGroup = { tag: string; items: PreparedItemOption[] };
type OrderCounts = { buy: number; sell: number; total: number };
type ClaimSummary = {
  entity_id?: bigint | number | string;
  name?: string;
  location?: unknown;
};
type MarketTreeGroupNode = NuxtTreeItem & {
  kind: "group";
  key: string;
  label: string;
  count: number;
  children: MarketTreeNode[];
};
type MarketTreeItemNode = NuxtTreeItem & {
  kind: "item";
  key: string;
  label: string;
  option: PreparedItemOption;
  stats?: MarketOrderStats;
  onSelect?: (event: Event) => void;
};
type MarketTreeNode = MarketTreeGroupNode | MarketTreeItemNode;

const createEmptyMarketOrders = (): MarketOrdersResponse => ({
  buy_orders: {},
  sell_orders: {},
});

const search = ref<string>("");
const selectedItemKeys = ref<string[]>([]);
const starredKeys = ref<string[]>([]);
const filterHasBuyOrders = ref(false);
const filterHasSellOrders = ref(false);
const STAR_STORAGE_KEY = "bitcraft.market.starred-item-keys.v1";

const route = useRoute();
const router = useRouter();

if (route.query.search) {
  search.value = route.query.search as string;
}

if (route.query.items) {
  selectedItemKeys.value = String(route.query.items)
    .split(",")
    .map((value) => value.trim())
    .filter(Boolean);
}

if (import.meta.client) {
  try {
    const raw = localStorage.getItem(STAR_STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (Array.isArray(parsed)) {
        starredKeys.value = parsed.filter((value): value is string => typeof value === "string");
      }
    }
  } catch {
    starredKeys.value = [];
  }
}

const { data: itemOptions } = await useLazyFetchMsPack<MarketItemCargoDescResponse>(
  () => {
    return `/market/item_cargo_desc`;
  },
  { deep: false },
);

const { data: marketStatsData, pending: marketStatsPending } =
  await useLazyFetchMsPack<MarketOrderStatsResponse>(
    () => {
      return `/market`;
    },
    { deep: false },
  );

const {
  data: selectedMarketOrdersData,
  pending: selectedOrdersPending,
  refresh: refreshSelectedOrders,
} = await useLazyFetchMsPack<MarketOrdersResponse>(
  () => {
    return `/market/orders`;
  },
  {
    deep: false,
    immediate: false,
    onRequest: ({ options }) => {
      const items = selectedItemKeys.value.join(",");
      options.query = items ? { items } : {};
    },
  },
);

const isMarketPending = computed(() => marketStatsPending.value || selectedOrdersPending.value);
const showPending = useDelayedPending(isMarketPending, 150);

const marketOrders = shallowRef<MarketOrdersResponse>(createEmptyMarketOrders());
const itemGroups = shallowRef<ItemGroup[]>([]);
const itemNameByKey = shallowRef(new Map<string, string>());
const itemOptionByKey = shallowRef(new Map<string, PreparedItemOption>());
const itemTagByKey = shallowRef(new Map<string, string>());
const openGroupTags = ref<string[]>([]);

watch(
  selectedMarketOrdersData,
  (value) => {
    marketOrders.value = value ?? createEmptyMarketOrders();
  },
  { immediate: true },
);

watch(
  itemOptions,
  (response) => {
    if (!response) {
      itemGroups.value = [];
      itemNameByKey.value = new Map();
      itemOptionByKey.value = new Map();
      itemTagByKey.value = new Map();
      return;
    }

    const groups: ItemGroup[] = [];
    const names = new Map<string, string>();
    const options = new Map<string, PreparedItemOption>();
    const tagsByKey = new Map<string, string>();

    for (const key in response.item_name_by_key) {
      names.set(key, response.item_name_by_key[key]);
    }

    for (const defaultTag in response.items_grouped) {
      const rawItems = response.items_grouped[defaultTag];
      if (!Array.isArray(rawItems) || rawItems.length === 0) {
        continue;
      }

      const normalizedItems: PreparedItemOption[] = [];

      for (const rawItem of rawItems) {
        const normalizedItem = {
          ...rawItem,
          tag: rawItem.tag || defaultTag,
          searchLabel: rawItem.label.toLowerCase(),
        } satisfies PreparedItemOption;

        normalizedItems.push(normalizedItem);
        options.set(normalizedItem.key, normalizedItem);
        tagsByKey.set(normalizedItem.key, `group:${normalizedItem.tag || defaultTag || "Other"}`);

        if (!names.has(normalizedItem.key)) {
          names.set(normalizedItem.key, normalizedItem.name);
        }
      }

      normalizedItems.sort((a, b) => {
        if (a.tier !== b.tier) {
          return a.tier - b.tier;
        }

        if (a.rarity !== b.rarity) {
          return raritySort(a.rarity, b.rarity);
        }

        return a.label.localeCompare(b.label);
      });

      groups.push({ tag: defaultTag || "Other", items: normalizedItems });
    }

    groups.sort((a, b) => a.tag.localeCompare(b.tag));

    itemGroups.value = groups;
    itemNameByKey.value = names;
    itemOptionByKey.value = options;
    itemTagByKey.value = tagsByKey;
  },
  { immediate: true },
);

const getOrderCounts = (key: string) => {
  return marketStatsData.value?.order_counts[key];
};

const formatStatAmount = (value: bigint | number | null | undefined) => {
  if (value == null) {
    return "0";
  }

  return value.toLocaleString();
};

const formatStatPrice = (value: number | null | undefined) => {
  if (value == null) {
    return "-";
  }

  return Math.round(value).toLocaleString();
};

const treeItems = computed(() => {
  if (!itemGroups.value.length) {
    return [] as MarketTreeNode[];
  }

  const starredGroups: MarketTreeGroupNode[] = [];
  const starredItems: MarketTreeItemNode[] = [];
  const grouped: MarketTreeGroupNode[] = [];
  const term = search.value.trim().toLowerCase();
  const hasSearch = term.length > 0;
  const hasBuyFilter = filterHasBuyOrders.value;
  const hasSellFilter = filterHasSellOrders.value;

  for (const group of itemGroups.value) {
    const filteredItems: MarketTreeItemNode[] = [];

    for (const option of group.items) {
      const stats = getOrderCounts(option.key);
      const matchesSearch = !hasSearch || option.searchLabel.includes(term);

      if (!matchesSearch) {
        continue;
      }

      if (hasBuyFilter || hasSellFilter) {
        const hasBuy = Number(stats?.buy ?? 0) > 0;
        const hasSell = Number(stats?.sell ?? 0) > 0;

        if (hasBuyFilter && hasSellFilter) {
          if (!hasBuy && !hasSell) {
            continue;
          }
        } else if (hasBuyFilter) {
          if (!hasBuy) {
            continue;
          }
        } else if (!hasSell) {
          continue;
        }
      }

      const itemNode: MarketTreeItemNode = {
        kind: "item",
        key: option.key,
        label: option.name,
        option,
        stats,
        onSelect: (event: Event) => event.preventDefault(),
      };

      if (starredKeys.value.includes(option.key)) {
        starredItems.push(itemNode);
        continue;
      }

      filteredItems.push(itemNode);
    }

    if (filteredItems.length) {
      const groupNode: MarketTreeGroupNode = {
        kind: "group",
        key: `group:${group.tag}`,
        label: group.tag,
        count: filteredItems.length,
        tag: group.tag,
        children: filteredItems,
        onSelect: (event: Event) => event.preventDefault(),
      };

      if (starredKeys.value.includes(groupNode.key)) {
        starredGroups.push(groupNode);
        continue;
      }

      grouped.push(groupNode);
    }
  }

  const result: MarketTreeNode[] = [];

  if (starredGroups.length) {
    result.push(...starredGroups);
  }

  if (starredItems.length) {
    result.push({
      kind: "group",
      key: "group:Star",
      label: "Star",
      count: starredItems.length,
      tag: "Star",
      children: starredItems,
      onSelect: (event: Event) => event.preventDefault(),
    });
  }

  result.push(...grouped);

  return result;
});

const expandedTreeKeys = computed(() => {
  if (search.value.trim().length > 0) {
    return treeItems.value
      .filter((item): item is MarketTreeGroupNode => item.kind === "group")
      .map((item) => item.key);
  }

  return openGroupTags.value;
});

const visibleTreeRows = computed(() => {
  const rows: MarketTreeNode[] = [];
  const expandedKeys = new Set(expandedTreeKeys.value);

  const visit = (items: MarketTreeNode[]) => {
    for (const item of items) {
      rows.push(item);

      if (item.kind === "group" && expandedKeys.has(item.key)) {
        visit(item.children);
      }
    }
  };

  visit(treeItems.value);

  return rows;
});

const flattenTreeItems = (items: MarketTreeNode[]): MarketTreeItemNode[] => {
  const result: MarketTreeItemNode[] = [];

  for (const item of items) {
    if (item.kind === "item") {
      result.push(item);
      continue;
    }

    result.push(...flattenTreeItems(item.children));
  }

  return result;
};

const selectedTreeItems = computed(() => {
  if (!selectedItemKeys.value.length) {
    return [] as MarketTreeItemNode[];
  }

  const selectedKeys = new Set(selectedItemKeys.value);

  return flattenTreeItems(treeItems.value).filter((item) => selectedKeys.has(item.key));
});

const selectedItems = computed(() => {
  if (!selectedItemKeys.value.length) {
    return [];
  }

  const result: PreparedItemOption[] = [];

  for (const key of selectedItemKeys.value) {
    const option = itemOptionByKey.value.get(key);
    if (option) {
      const stats = getOrderCounts(key);
      if (stats) {
        option.stats = stats;
      }
      result.push(option);
    }
  }

  return result;
});

watch(
  selectedItemKeys,
  (keys) => {
    if (!import.meta.client) {
      marketOrders.value = createEmptyMarketOrders();
      return;
    }

    if (!keys.length) {
      marketOrders.value = createEmptyMarketOrders();
      return;
    }

    refreshSelectedOrders();
  },
  { immediate: true },
);

watch(
  selectedItemKeys,
  (keys) => {
    if (!keys.length) {
      return;
    }

    const nextTags = new Set(openGroupTags.value);

    for (const key of keys) {
      const tag = itemTagByKey.value.get(key);
      if (tag) {
        nextTags.add(tag);
      }
    }

    if (nextTags.size !== openGroupTags.value.length) {
      openGroupTags.value = [...nextTags];
    }
  },
  { immediate: true },
);

const upsertOrder = (book: keyof MarketOrdersResponse, message: AuctionListingState) => {
  const key = `${message.item_type}:${message.item_id}`;
  if (!selectedItemKeys.value.includes(key)) {
    return;
  }

  const currentMarketOrders = marketOrders.value;
  if (!currentMarketOrders) {
    return;
  }

  const orderBook = currentMarketOrders[book];
  const countDelta: OrderCounts =
    book === "buy_orders" ? { buy: 1, sell: 0, total: 1 } : { buy: 0, sell: 1, total: 1 };

  if (!orderBook[key]) {
    orderBook[key] = [message];
    marketStatsData.value.order_counts[key] = {
      buy: countDelta.buy,
      sell: countDelta.sell,
      total: 1,
      buy_price_highest: null,
      buy_price_lowest: null,
      buy_amount_lowest: null,
      buy_amount_highest: null,
      sell_price_highest: null,
      sell_price_lowest: null,
      sell_amount_lowest: null,
      sell_amount_highest: null,
    };
  } else {
    const index = orderBook[key].findIndex((order) => order.entity_id === message.entity_id);

    if (index > -1) {
      orderBook[key][index] = message;
    } else {
      orderBook[key].push(message);

      const existing = marketStatsData.value?.order_counts[key];
      marketStatsData.value.order_counts[key] = {
        buy: (existing?.buy ?? 0) + countDelta.buy,
        sell: (existing?.sell ?? 0) + countDelta.sell,
        total: (existing?.total ?? 0) + 1,
        buy_price_highest: existing?.buy_price_highest ?? null,
        buy_price_lowest: existing?.buy_price_lowest ?? null,
        buy_amount_lowest: existing?.buy_amount_lowest ?? null,
        buy_amount_highest: existing?.buy_amount_highest ?? null,
        sell_price_highest: existing?.sell_price_highest ?? null,
        sell_price_lowest: existing?.sell_price_lowest ?? null,
        sell_amount_lowest: existing?.sell_amount_lowest ?? null,
        sell_amount_highest: existing?.sell_amount_highest ?? null,
      };
    }
  }

  triggerRef(marketOrders);
  triggerRef(marketStatsData);
};

const removeOrder = (book: keyof MarketOrdersResponse, message: AuctionListingState) => {
  const key = `${message.item_type}:${message.item_id}`;
  if (!selectedItemKeys.value.includes(key)) {
    return;
  }

  const currentMarketOrders = marketOrders.value;
  if (!currentMarketOrders) {
    return;
  }

  const orderBook = currentMarketOrders[book];

  if (!orderBook[key]) {
    return;
  }

  const index = orderBook[key].findIndex((order) => order.entity_id === message.entity_id);

  if (index > -1) {
    orderBook[key].splice(index, 1);

    if (orderBook[key].length === 0) {
      delete orderBook[key];
    }

    const existing = marketStatsData?.value.order_counts[key];
    if (existing) {
      const nextBuy = book === "buy_orders" ? Math.max(0, existing.buy - 1) : existing.buy;
      const nextSell = book === "sell_orders" ? Math.max(0, existing.sell - 1) : existing.sell;
      const nextTotal = nextBuy + nextSell;

      if (nextTotal === 0) {
        delete marketStatsData.value.order_counts[key];
      } else {
        marketStatsData.value.order_counts[key] = {
          buy: nextBuy,
          sell: nextSell,
          total: nextTotal,
          buy_price_highest: existing.buy_price_highest,
          buy_price_lowest: existing.buy_price_lowest,
          buy_amount_lowest: existing.buy_amount_lowest,
          buy_amount_highest: existing.buy_amount_highest,
          sell_price_highest: existing.sell_price_highest,
          sell_price_lowest: existing.sell_price_lowest,
          sell_amount_lowest: existing.sell_amount_lowest,
          sell_amount_highest: existing.sell_amount_highest,
        };
      }
    }

    triggerRef(marketOrders);
    triggerRef(marketStatsData);
  }
};

registerWebsocketMessageHandler("InsertBuyOrder", ["insert_buy_order"], (message) => {
  upsertOrder("buy_orders", message);
});

registerWebsocketMessageHandler("UpdateBuyOrder", ["update_buy_order"], (message) => {
  upsertOrder("buy_orders", message);
});

registerWebsocketMessageHandler("RemoveBuyOrder", ["remove_buy_order"], (message) => {
  removeOrder("buy_orders", message);
});

registerWebsocketMessageHandler("InsertSellOrder", ["insert_sell_order"], (message) => {
  upsertOrder("sell_orders", message);
});

registerWebsocketMessageHandler("UpdateSellOrder", ["update_sell_order"], (message) => {
  upsertOrder("sell_orders", message);
});

registerWebsocketMessageHandler("RemoveSellOrder", ["remove_sell_order"], (message) => {
  removeOrder("sell_orders", message);
});

watchThrottled(
  () => [search.value, selectedItemKeys.value.join(",")],
  () => {
    const currentSearch = typeof route.query.search === "string" ? route.query.search : "";
    const currentItems = typeof route.query.items === "string" ? route.query.items : "";
    const nextSearch = search.value;
    const nextItems = selectedItemKeys.value.join(",");

    if (currentSearch === nextSearch && currentItems === nextItems) {
      return;
    }

    const query: Record<string, string> = {};

    if (nextSearch) {
      query.search = nextSearch;
    }

    if (nextItems) {
      query.items = nextItems;
    }

    router.replace({ query });
  },
  { throttle: 150 },
);

const sellOrders = computed(() => {
  if (selectedItemKeys.value.length === 0) {
    return [];
  }

  const orders: AuctionListingState[] = [];

  for (const key of selectedItemKeys.value) {
    const itemOrders = marketOrders.value.sell_orders[key];
    if (itemOrders?.length) {
      orders.push(...itemOrders);
    }
  }

  return orders.sort((a, b) => a.price_threshold - b.price_threshold);
});

const buyOrders = computed(() => {
  if (selectedItemKeys.value.length === 0) {
    return [];
  }

  const orders: AuctionListingState[] = [];

  for (const key of selectedItemKeys.value) {
    const itemOrders = marketOrders.value.buy_orders[key];
    if (itemOrders?.length) {
      orders.push(...itemOrders);
    }
  }

  return orders.sort((a, b) => b.price_threshold - a.price_threshold);
});

const visibleClaimIds = computed(() => {
  const ids = new Set<string>();

  for (const order of sellOrders.value) {
    ids.add(order.claim_entity_id.toString());
  }

  for (const order of buyOrders.value) {
    ids.add(order.claim_entity_id.toString());
  }

  return [...ids];
});

const { data: claimNames } = await useLazyFetchMsPack<Map<number, ClaimSummary>>(
  () => `/claims/names`,
);

const sellColumns: TableColumn<AuctionListingState>[] = [
  { id: "item_id", header: "Item", meta: { class: { th: "text-center", td: "text-center" } } },
  {
    id: "quantity",
    header: "Quantity",
    meta: { class: { th: "text-center", td: "text-center" } },
  },
  {
    id: "price_threshold",
    header: "Price",
    meta: { class: { th: "text-center", td: "text-center" } },
  },
  {
    id: "claim_entity_id",
    header: "Claim",
    meta: { class: { th: "text-center", td: "text-center" } },
  },
  { id: "region", header: "Region", meta: { class: { th: "text-center", td: "text-center" } } },
];

const buyColumns = sellColumns;

const toggleItem = (key: string) => {
  if (selectedItemKeys.value.includes(key)) {
    selectedItemKeys.value = selectedItemKeys.value.filter((value) => value !== key);
    return;
  }

  selectedItemKeys.value = [...selectedItemKeys.value, key];
};

const getGroupStarKey = (tag: string) => {
  return `group:${tag}`;
};

const isStarred = (key: string) => {
  return starredKeys.value.includes(key);
};

const toggleStar = (key: string) => {
  if (isStarred(key)) {
    starredKeys.value = starredKeys.value.filter((value) => value !== key);
    return;
  }

  starredKeys.value = [...starredKeys.value, key];
};

const toggleGroupStar = (tag: string) => {
  toggleStar(getGroupStarKey(tag));
};

const handleTreeSelection = (items: MarketTreeNode[]) => {
  selectedItemKeys.value = items
    .filter((item): item is MarketTreeItemNode => item.kind === "item")
    .map((item) => item.key);
};

const handleTreeExpanded = (keys: string[]) => {
  if (search.value.trim().length > 0) {
    return;
  }

  openGroupTags.value = keys;
};

watch(
  starredKeys,
  (value) => {
    if (!import.meta.client) return;
    localStorage.setItem(STAR_STORAGE_KEY, JSON.stringify(value));
  },
  { deep: false },
);

useSeoMeta({
  title: "Market",
  description: "Live Bitcraft market buy and sell orders",
});

import { getPaginationRowModel } from "@tanstack/vue-table";

const tableSellOrders = useTemplateRef("table-sell-orders");

const paginationSellOrders = ref({
  pageIndex: 0,
  pageSize: 15,
});

const tableBuyOrders = useTemplateRef("table-buy-orders");

const paginationBuyOrders = ref({
  pageIndex: 0,
  pageSize: 15,
});
</script>

<template>
  <UContainer class="w-full max-w-none py-8">
    <div class="flex flex-col gap-6">
      <div class="flex flex-col gap-2">
        <p class="text-xs uppercase tracking-[0.2em] text-gray-500 dark:text-gray-400">Market</p>
        <h1 class="text-2xl font-semibold tracking-tight text-gray-900 dark:text-gray-100">
          Live order book
        </h1>
        <p class="text-sm text-gray-600 dark:text-gray-300">
          Pick one or more items to compare sell and buy orders.
        </p>
      </div>

      <UCard v-if="selectedItems.length" :ui="{ body: 'p-3' }">
        <div class="flex flex-col gap-2">
          <div class="flex items-center justify-between gap-2">
            <p
              class="text-xs font-semibold uppercase tracking-[0.12em] text-gray-600 dark:text-gray-300"
            >
              Selected items
            </p>
            <UButton size="xs" color="neutral" variant="ghost" @click="selectedItemKeys = []">
              Clear all
            </UButton>
          </div>
          <div class="flex flex-wrap gap-2">
            <UButton
              v-for="item in selectedItems"
              :key="item.key"
              size="xs"
              color="primary"
              variant="soft"
              @click="toggleItem(item.key)"
            >
              <span class="truncate">{{ item.name }}</span>
              <span class="ml-1 text-[10px]" :class="rarityToTextClass(item.rarity)">
                {{ item.rarity }}
              </span>
              <span class="ml-1 text-[10px] font-semibold" :class="tierToTextClass(item.tier)">
                T{{ item.tier }}
              </span>
              <span
                v-if="item.stats"
                class="mt-0.5 flex flex-col text-[10px] leading-4 text-gray-500 dark:text-gray-400"
              >
                <span
                  v-if="item.stats.sell"
                  class="grid grid-cols-[2.5rem_max-content_max-content_max-content] gap-x-1 truncate justify-center"
                >
                  <span>Sell: {{ formatStatAmount(item.stats.sell) }}</span>
                  <span>Low: {{ formatStatPrice(item.stats.sell_price_lowest) }}</span>
                  <span>High: {{ formatStatPrice(item.stats.sell_price_highest) }}</span>
                </span>
                <span
                  v-if="item.stats.buy"
                  class="grid grid-cols-[2.5rem_max-content_max-content_max-content] gap-x-1 truncate justify-center"
                >
                  <span>Buy: {{ formatStatAmount(item.stats.buy) }}</span>
                  <span>Low: {{ formatStatPrice(item.stats.buy_price_lowest) }}</span>
                  <span>High: {{ formatStatPrice(item.stats.buy_price_highest) }}</span>
                </span>
              </span>
              <UIcon name="i-lucide-x" class="ml-1 h-3 w-3" />
            </UButton>
          </div>
        </div>
      </UCard>

      <UProgress v-if="showPending" color="neutral" />

      <div class="grid grid-cols-1 gap-4 lg:grid-cols-5">
        <div class="lg:order-2 lg:col-span-4">
          <template v-if="selectedItems.length">
            <div class="grid grid-cols-1 gap-4 xl:grid-cols-2">
              <UCard :ui="{ header: 'p-4', body: 'p-0' }">
                <template #header>
                  <div class="flex items-center justify-between">
                    <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100">
                      Sell Orders
                    </h2>
                    <UBadge color="neutral" variant="soft">{{ sellOrders.length }}</UBadge>
                  </div>
                </template>
                <UTable
                  ref="table-sell-orders"
                  v-model:pagination="paginationSellOrders"
                  :columns="sellColumns"
                  :data="sellOrders"
                  :pagination-options="{
                    getPaginationRowModel: getPaginationRowModel(),
                  }"
                >
                  <template #item_id-cell="{ row }">
                    <div class="flex items-center justify-center gap-2">
                      <BitcraftInventoryImg
                        :item="
                          itemOptionByKey.get(`${row.original.item_type}:${row.original.item_id}`)
                        "
                        :width="24"
                        :height="24"
                        class="shrink-0"
                      />
                      <span class="truncate">
                        {{
                          itemNameByKey.get(`${row.original.item_type}:${row.original.item_id}`) ||
                          `#${row.original.item_id}`
                        }}
                      </span>
                      <span
                        class="truncate"
                        :class="
                          rarityToTextClass(
                            itemOptionByKey.get(`${row.original.item_type}:${row.original.item_id}`)
                              ?.rarity,
                          )
                        "
                      >
                        {{
                          itemOptionByKey.get(`${row.original.item_type}:${row.original.item_id}`)
                            ?.rarity
                        }}
                      </span>
                    </div>
                  </template>
                  <template #quantity-cell="{ row }">{{
                    row.original.quantity.toLocaleString()
                  }}</template>
                  <template #price_threshold-cell="{ row }">{{
                    row.original.price_threshold.toLocaleString()
                  }}</template>
                  <template #claim_entity_id-cell="{ row }">
                    <NuxtLink
                      :to="{
                        name: 'claims-id',
                        params: { id: row.original.claim_entity_id.toString() },
                      }"
                      class="text-primary-500 hover:underline"
                    >
                      {{
                        claimNames?.[row.original.claim_entity_id].name ||
                        row.original.claim_entity_id.toString()
                      }}
                    </NuxtLink>
                  </template>
                  <template #region-cell="{ row }">R{{ row.original.region }}</template>
                </UTable>
                <div class="flex justify-end border-t border-default pt-4 px-4">
                  <UPagination
                    :page="(tableSellOrders?.tableApi?.getState().pagination.pageIndex || 0) + 1"
                    :items-per-page="tableSellOrders?.tableApi?.getState().pagination.pageSize"
                    :total="tableSellOrders?.tableApi?.getFilteredRowModel().rows.length"
                    @update:page="(p) => tableSellOrders?.tableApi?.setPageIndex(p - 1)"
                  />
                </div>
              </UCard>

              <UCard :ui="{ header: 'p-4', body: 'p-0' }">
                <template #header>
                  <div class="flex items-center justify-between">
                    <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100">
                      Buy Orders
                    </h2>
                    <UBadge color="neutral" variant="soft">{{ buyOrders.length }}</UBadge>
                  </div>
                </template>
                <UTable
                  ref="table-buy-orders"
                  v-model:pagination="paginationBuyOrders"
                  :columns="buyColumns"
                  :data="buyOrders"
                  :pagination-options="{
                    getPaginationRowModel: getPaginationRowModel(),
                  }"
                >
                  <template #item_id-cell="{ row }">
                    <div class="flex items-center justify-center gap-2">
                      <BitcraftInventoryImg
                        :item="
                          itemOptionByKey.get(`${row.original.item_type}:${row.original.item_id}`)
                        "
                        :width="24"
                        :height="24"
                        class="shrink-0"
                      />
                      <span class="truncate">
                        {{
                          itemNameByKey.get(`${row.original.item_type}:${row.original.item_id}`) ||
                          `#${row.original.item_id}`
                        }}
                      </span>
                      <span
                        class="truncate"
                        :class="
                          rarityToTextClass(
                            itemOptionByKey.get(`${row.original.item_type}:${row.original.item_id}`)
                              ?.rarity,
                          )
                        "
                      >
                        {{
                          itemOptionByKey.get(`${row.original.item_type}:${row.original.item_id}`)
                            ?.rarity
                        }}
                      </span>
                    </div>
                  </template>
                  <template #quantity-cell="{ row }">{{
                    row.original.quantity.toLocaleString()
                  }}</template>
                  <template #price_threshold-cell="{ row }">{{
                    row.original.price_threshold.toLocaleString()
                  }}</template>
                  <template #claim_entity_id-cell="{ row }">
                    <NuxtLink
                      :to="{
                        name: 'claims-id',
                        params: { id: row.original.claim_entity_id.toString() },
                      }"
                      class="text-primary-500 hover:underline"
                    >
                      {{
                        claimNames?.[row.original.claim_entity_id].name ||
                        row.original.claim_entity_id.toString()
                      }}
                    </NuxtLink>
                  </template>
                  <template #region-cell="{ row }">R{{ row.original.region }}</template>
                </UTable>
                <div class="flex justify-end border-t border-default pt-4 px-4">
                  <UPagination
                    :page="(tableBuyOrders?.tableApi?.getState().pagination.pageIndex || 0) + 1"
                    :items-per-page="tableBuyOrders?.tableApi?.getState().pagination.pageSize"
                    :total="tableBuyOrders?.tableApi?.getFilteredRowModel().rows.length"
                    @update:page="(p) => tableBuyOrders?.tableApi?.setPageIndex(p - 1)"
                  />
                </div>
              </UCard>
            </div>
          </template>
          <UEmpty
            v-else
            icon="i-lucide-store"
            title="Select items to view orders"
            description="Use the item groups on the left to load buy and sell orders."
          />
        </div>

        <UCard class="h-fit lg:order-1 lg:sticky lg:top-4" :ui="{ body: 'p-4' }">
          <div class="flex flex-col gap-3">
            <div class="flex items-center justify-between gap-2">
              <h2
                class="text-sm font-semibold uppercase tracking-[0.12em] text-gray-700 dark:text-gray-200"
              >
                Items
              </h2>
              <UButton
                v-if="selectedItemKeys.length"
                size="xs"
                color="neutral"
                variant="ghost"
                @click="selectedItemKeys = []"
              >
                Clear ({{ selectedItemKeys.length }})
              </UButton>
            </div>

            <UInput
              v-model="search"
              icon="i-lucide-search"
              placeholder="Search items and cargo"
              variant="outline"
            />

            <div class="flex flex-wrap items-center gap-3">
              <USwitch v-model="filterHasBuyOrders" label="Has buy orders" />
              <USwitch v-model="filterHasSellOrders" label="Has sell orders" />
            </div>
            <UTree
              v-if="treeItems.length"
              class="max-h-[60vh] overflow-y-auto pr-1"
              :items="treeItems"
              :model-value="selectedTreeItems"
              :expanded="expandedTreeKeys"
              :get-key="(item) => item.key"
              multiple
              color="neutral"
              selection-behavior="toggle"
              :virtualize="{ estimateSize: 32, overscan: 20 }"
              @update:model-value="handleTreeSelection"
              @update:expanded="handleTreeExpanded"
            >
              <template #item="{ item, selected, handleSelect, handleToggle }">
                <div v-if="item.kind === 'group'" class="flex w-full items-center gap-1">
                  <UButton
                    size="xs"
                    color="neutral"
                    variant="ghost"
                    block
                    class="flex-1 justify-between rounded-md border border-gray-200 text-sm font-semibold text-gray-800 dark:border-gray-800 dark:text-gray-100"
                  >
                    <span class="truncate">{{ item.label }}</span>
                    <UBadge color="neutral" variant="soft" size="xs">{{ item.count }}</UBadge>
                  </UButton>
                  <UButton
                    v-if="item.key != 'group:Star'"
                    size="xs"
                    color="warning"
                    :variant="isStarred(item.key) ? 'solid' : 'ghost'"
                    icon="i-lucide-star"
                    :aria-label="isStarred(item.key) ? 'Unstar group' : 'Star group'"
                    @click.stop="toggleGroupStar(item.tag)"
                  />
                </div>
                <div v-else class="flex w-full items-center gap-1">
                  <UButton
                    size="xs"
                    color="neutral"
                    :variant="selected ? 'outline' : 'soft'"
                    block
                    class="flex-1 justify-start"
                    @click="handleSelect"
                  >
                    <BitcraftInventoryImg
                      :item="item.option"
                      :width="30"
                      :height="20"
                      class="shrink-0"
                    />
                    <span class="min-w-0 flex-1">
                      <span class="truncate">{{ item.option.name }}</span>
                      <span class="text-[10px]" :class="rarityToTextClass(item.option.rarity)">
                        {{ item.option.rarity }}
                      </span>
                      <span
                        class="shrink-0 text-[10px] font-semibold"
                        :class="tierToTextClass(item.option.tier)"
                      >
                        T{{ item.option.tier }}
                      </span>
                    </span>
                  </UButton>
                  <UButton
                    size="xs"
                    color="warning"
                    :variant="isStarred(item.key) ? 'solid' : 'ghost'"
                    icon="i-lucide-star"
                    :aria-label="isStarred(item.key) ? 'Unstar item' : 'Star item'"
                    @click.stop="toggleStar(item.key)"
                  />
                </div>
              </template>
            </UTree>
            <p v-else class="text-sm text-gray-500 dark:text-gray-400">
              No items match your search.
            </p>
          </div>
        </UCard>
      </div>
    </div>
  </UContainer>
</template>
