<script setup lang="ts">
import { watchDebounced, watchThrottled } from "@vueuse/shared";
import { iconAssetUrlNameRandom } from "~/composables/iconAssetName";
import type { ItemsAndCargoResponse } from "~/types/ItemsAndCargoResponse";
import type { MetaResponse } from "~/types/MetaResponse";
import { rarityToTextClass, tierToBorderClass, tierToTextClass, useDelayedPending } from "~/utils";

const page = ref(1);
const perPage = 20;

const tag = ref<string | null>(null);
const tier = ref<number | null>(null);
const search = ref<string | null>("");
const debouncedSearch = ref<string | null>("");

const route = useRoute();
const router = useRouter();

debouncedSearch.value = (route.query.search as string) ?? "";
search.value = debouncedSearch.value;
tag.value = (route.query.tag as string) ?? null;

if (route.query.tier) {
  tier.value = parseInt(route.query.tier);
}
if (route.query.page) {
  page.value = parseInt(route.query.page);
}

const { data, pending, refresh } = await useLazyFetchMsPack<ItemsAndCargoResponse>(
  () => {
    return `/api/bitcraft/itemsAndCargo`;
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

      if (tag.value) {
        options.query.tag = tag.value;
      }

      if (tier.value) {
        options.query.tier = tier.value;
      }

      if (perPage) {
        options.query.per_page = perPage;
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

const showPending = useDelayedPending(pending, 150);

const { data: metaData } = await useLazyFetchMsPack<MetaResponse>(() => {
  return `/api/bitcraft/itemsAndCargo/meta`;
});

const tagOptions = computed(() => {
  const tags = metaData.value?.tags ?? [];
  return [
    { label: "All tags", value: null },
    ...tags.map((value: string) => ({ label: value, value })),
  ];
});

const tierOptions = computed(() => {
  const tiers = metaData.value?.tiers ?? [];
  return [
    { label: "All tiers", value: null },
    ...tiers.map((value: number) => ({ label: `Tier ${value}`, value })),
  ];
});

const imageErrors = ref(new Set<number>());
const itemImageElements = new Map<number, HTMLImageElement>();
const itemIconUrls = useState<Record<string, string | null>>("items-page-icon-urls", () => ({}));

const itemIconCacheKey = (item: ItemsAndCargoResponse["items"][number]) => {
  return `${item.id}:${item.icon_asset_name ?? ""}`;
};

const iconForItem = (item: ItemsAndCargoResponse["items"][number]) => {
  if (!item.icon_asset_name) {
    return null;
  }

  const cacheKey = itemIconCacheKey(item);
  const cachedIconUrl = itemIconUrls.value[cacheKey];

  if (cachedIconUrl !== undefined) {
    return cachedIconUrl;
  }

  const icon = iconAssetUrlNameRandom(item.icon_asset_name);
  const iconUrl = icon.show ? icon.url : null;

  itemIconUrls.value[cacheKey] = iconUrl;

  return iconUrl;
};

const onImageError = (id: number) => {
  imageErrors.value.add(id);
};

const hasImageError = (id: number) => imageErrors.value.has(id);

const items = computed(() => {
  return (data.value?.items ?? []).map((item) => ({
    ...item,
    iconUrl: iconForItem(item),
  }));
});

const syncImageErrorState = (id: number) => {
  const imageElement = itemImageElements.get(id);

  if (!imageElement?.complete) {
    return;
  }

  if (imageElement.naturalWidth === 0) {
    onImageError(id);
  }
};

const setItemImageElement = (id: number, element: Element | null) => {
  if (element instanceof HTMLImageElement) {
    itemImageElements.set(id, element);
    syncImageErrorState(id);
    return;
  }

  itemImageElements.delete(id);
};

const syncVisibleImageErrors = async () => {
  if (import.meta.server) {
    return;
  }

  await nextTick();

  for (const item of items.value) {
    if (item.iconUrl && !hasImageError(item.id)) {
      syncImageErrorState(item.id);
    }
  }
};

const isFiltering = computed(() => {
  return Boolean(debouncedSearch.value || tag.value || tier.value);
});

const emptyTitle = computed(() => {
  return isFiltering.value ? "No items match" : "No items found";
});

const emptyDescription = computed(() => {
  return isFiltering.value
    ? "Try adjusting your search or filters."
    : "Try changing your search criteria.";
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
    search.value = debouncedSearch.value;

    refresh();
  },
  { debounce: 100, maxWait: 200 },
);

watchThrottled(
  () => [tag.value, tier.value, search.value],
  (value, oldValue) => {
    if (value[1] !== oldValue[1] || value[2] !== oldValue[2] || value[3] !== oldValue[3]) {
      page.value = 1;
    }

    refresh();
  },
  { throttle: 50 },
);

watch(
  items,
  () => {
    syncVisibleImageErrors();
  },
  { flush: "post" },
);

onMounted(() => {
  syncVisibleImageErrors();
});

useSeoMeta({
  title: () => `Items ${data.value?.total ?? 0} ${search.value ?? ""}`,
  description: "List of all the Items in the game",
});
</script>

<template>
  <UContainer class="w-full max-w-none py-8">
    <div class="flex flex-col gap-6">
      <div class="flex flex-col gap-3">
        <div class="flex flex-col gap-1">
          <p class="text-xs uppercase tracking-[0.2em] text-gray-500 dark:text-gray-400">Items</p>
          <div class="flex flex-wrap items-end justify-between gap-3">
            <div>
              <h1 class="text-2xl font-semibold tracking-tight text-gray-900 dark:text-gray-100">
                Item catalog
              </h1>
              <p class="text-sm text-gray-600 dark:text-gray-300">
                Browse every item with filters and paging.
              </p>
            </div>
            <div
              class="flex items-center gap-2 rounded-full border border-gray-200 px-3 py-1 text-xs text-gray-600 shadow-sm dark:border-gray-800 dark:text-gray-300"
            >
              <span>Total</span>
              <span class="font-semibold text-gray-900 dark:text-gray-100">
                {{ Number(data?.total ?? 0).toLocaleString() }}
              </span>
            </div>
          </div>
        </div>

        <div class="grid grid-cols-1 gap-3 sm:grid-cols-3">
          <UInput
            v-model="debouncedSearch"
            icon="i-lucide-search"
            placeholder="Search items"
            variant="outline"
          />
          <USelect v-model="tag" :items="tagOptions" placeholder="All tags" variant="outline" />
          <USelect v-model="tier" :items="tierOptions" placeholder="All tiers" variant="outline" />
        </div>
      </div>
      <div class="flex min-h-[44px] justify-center pb-4" :class="pending ? 'opacity-60' : ''">
        <UPagination
          v-model:page="page"
          :total="Number(data?.total ?? 0)"
          :items-per-page="perPage"
          size="sm"
          :disabled="showPending"
          @update:page="changePage"
        />
      </div>

      <UProgress v-if="showPending" color="neutral" />

      <template v-if="data">
        <template v-if="(data?.items ?? []).length">
          <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-5">
            <UCard
              v-for="item in items"
              :key="item.id"
              :ui="{ body: 'p-2' }"
              :class="['border-l-4', tierToBorderClass(item.tier)]"
            >
              <div class="flex items-start justify-between gap-4">
                <div class="flex flex-col items-start gap-2">
                  <NuxtLink
                    :to="{ name: 'items-type-id', params: { id: item.id, type: item.type } }"
                    class="text-sm font-semibold text-gray-900 hover:text-gray-700 dark:text-gray-100 dark:hover:text-gray-200"
                  >
                    {{ item.name }}
                  </NuxtLink>
                  <img
                    v-if="item.iconUrl && !hasImageError(item.id)"
                    :ref="(element) => setItemImageElement(item.id, element as Element | null)"
                    :src="item.iconUrl"
                    :alt="item.name"
                    class="h-auto max-h-18 w-auto max-w-full rounded border border-gray-200 object-contain dark:border-gray-800"
                    loading="lazy"
                    @error="onImageError(item.id)"
                  />
                </div>
                <div class="flex flex-col items-end gap-1 text-xs text-gray-500 dark:text-gray-400">
                  <div class="uppercase tracking-widest">
                    {{ item.type }}
                  </div>
                  <div :class="tierToTextClass(item.tier)">Tier: {{ item.tier }}</div>
                  <div class="uppercase tracking-widest" :class="rarityToTextClass(item.rarity)">
                    Rarity: {{ item.rarity }}
                  </div>
                  <div>Tag: {{ item.tag }}</div>
                  <div>Volume: {{ item.volume }}</div>
                </div>
              </div>
            </UCard>
          </div>
        </template>
        <UEmpty
          v-else
          icon="i-lucide-package-search"
          :title="emptyTitle"
          :description="emptyDescription"
        />
      </template>
      <template v-else>
        <UEmpty
          icon="i-lucide-package-search"
          title="No items found"
          description="Try changing your search criteria."
        />
      </template>
    </div>
  </UContainer>
</template>
