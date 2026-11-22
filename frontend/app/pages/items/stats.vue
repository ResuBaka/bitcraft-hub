<script setup lang="ts">
import { refDebounced } from "@vueuse/core";
import { iconAssetUrlName } from "~/composables/iconAssetName";
import { rarityToTextClass, tierToBorderClass, tierToTextClass } from "~/utils";
import type { AllInventoryStatsResponse } from "~/types/AllInventoryStatsResponse";
import type { MetaResponse } from "~/types/MetaResponse";
import type { ItemExpended } from "~/types/ItemExpended";

const itemsPage = ref(1);
const cargoPage = ref(1);
const pageSize = 20;

const tag = ref<string | null>(null);
const tier = ref<number | null>(null);
const rarity = ref<string | null>(null);
const route = useRoute();
const search = ref(typeof route.query.search === "string" ? route.query.search : "");
const debouncedSearch = refDebounced(search, 150);
const searchText = computed(() => debouncedSearch.value?.trim().toLocaleLowerCase() ?? "");
tag.value = (route.query.tag as string) ?? null;
rarity.value = (route.query.rarity as string) ?? null;

if (route.query.tier) {
  tier.value = parseInt(route.query.tier);
}

const { data, pending } = await useLazyFetchMsPack<AllInventoryStatsResponse>(
  () => {
    return `/inventory/all_inventory_stats`;
  },
  {
    timeout: 15000,
    server: false,
  },
);

const { data: metaData } = await useLazyFetchMsPack<MetaResponse>(
  () => {
    return `/api/bitcraft/itemsAndCargo/meta`;
  },
  {
    timeout: 15000,
  },
);

const tagOptions = computed(() => {
  const tags = metaData.value?.tags ?? [];

  return [
    { label: "All tags", value: null },
    ...tags
      .filter((value: string) => value.length != 0)
      .map((value: string) => ({ label: value, value })),
  ];
});

const tierOptions = computed(() => {
  const tiers = metaData.value?.tiers ?? [];
  return [
    { label: "All tiers", value: null },
    ...tiers.map((value: number) => ({ label: `Tier ${value}`, value })),
  ];
});

const rarityOptions = computed(() => {
  const rarityOrder = ["Default", "Common", "Uncommon", "Rare", "Epic", "Legendary", "Mythic"];
  const rarities = new Set<string>();

  for (const value of [
    ...Object.values(data.value?.items ?? {}),
    ...Object.values(data.value?.cargo ?? {}),
  ]) {
    if (value[1]?.rarity) {
      rarities.add(value[1].rarity);
    }
  }

  return [
    { label: "All rarities", value: null },
    ...rarityOrder.filter((value) => rarities.has(value)).map((value) => ({ label: value, value })),
  ];
});

const items = computed(() => {
  if (!data || !data?.value) {
    return [];
  }
  const filtered = Object.values(data.value.items).filter((value) => {
    if (!value[1]) return false;
    if (tag.value && value[1]?.tag !== tag.value) {
      return false;
    }

    if (tier.value && value[1]?.tier !== tier.value) {
      return false;
    }

    if (rarity.value && value[1]?.rarity !== rarity.value) {
      return false;
    }

    if (!searchText.value) {
      return true;
    }

    if (value[1] == null) {
      return false;
    }

    if (value[1].name.toLocaleLowerCase().includes(searchText.value)) {
      return true;
    }

    if (value[1].rarity.toLocaleLowerCase().includes(searchText.value)) {
      return true;
    }

    return false;
  });

  if (!searchText.value && !tag.value && !tier.value && !rarity.value) {
    return filtered.slice(0, 19);
  }

  return filtered;
});

const cargo = computed(() => {
  if (!data || !data?.value) {
    return [];
  }
  const filtered = Object.values(data.value.cargo).filter((value) => {
    if (!value[1]) return false;
    if (tag.value && value[1]?.tag !== tag.value) {
      return false;
    }

    if (tier.value && value[1]?.tier !== tier.value) {
      return false;
    }

    if (rarity.value && value[1]?.rarity !== rarity.value) {
      return false;
    }

    if (!searchText.value) {
      return true;
    }

    if (value[1] == null) {
      return false;
    }

    if (value[1].name.toLocaleLowerCase().includes(searchText.value)) {
      return true;
    }

    if (value[1].rarity.toLocaleLowerCase().includes(searchText.value)) {
      return true;
    }

    return false;
  });

  if (!searchText.value && !tag.value && !tier.value && !rarity.value) {
    return filtered.slice(0, 19);
  }

  return filtered;
});

const numberFormat = new Intl.NumberFormat();

const regionsByQuantity = (regions: { [key: number]: bigint }) =>
  Object.entries(regions).sort((a, b) =>
    a[1] > b[1] ? -1 : a[1] < b[1] ? 1 : Number(a[0]) - Number(b[0]),
  );

const imageErrors = ref(new Set<number>());
const expandedRegions = ref(new Set<string>());
function toggleRegions(key: string, event: Event) {
  if ((event.currentTarget as HTMLDetailsElement).open) expandedRegions.value.add(key);
  else expandedRegions.value.delete(key);
}

const iconForItem = (item: ItemExpended | null) => {
  if (!item?.icon_asset_name) {
    return null;
  }

  const icon = iconAssetUrlName(item.icon_asset_name);
  return icon.show ? icon.url : null;
};
const currentItems = computed(() =>
  items.value.slice((itemsPage.value - 1) * pageSize, itemsPage.value * pageSize)
    .map((item) => ({ item, icon: iconForItem(item[1]) })),
);
const currentCargo = computed(() =>
  cargo.value.slice((cargoPage.value - 1) * pageSize, cargoPage.value * pageSize)
    .map((item) => ({ item, icon: iconForItem(item[1]) })),
);
watch([searchText, tag, tier, rarity], () => {
  itemsPage.value = 1;
  cargoPage.value = 1;
});
watch([currentItems, currentCargo], () => {
  const ids = new Set([...currentItems.value, ...currentCargo.value].map(({ item }) => item[1]?.id));
  for (const id of imageErrors.value) {
    if (!ids.has(id)) imageErrors.value.delete(id);
  }
  const keys = new Set([
    ...currentItems.value.map(({ item }) => `item:${item[1]?.id}`),
    ...currentCargo.value.map(({ item }) => `cargo:${item[1]?.id}`),
  ]);
  for (const key of expandedRegions.value) {
    if (!keys.has(key)) expandedRegions.value.delete(key);
  }
});

const onImageError = (id: number) => {
  imageErrors.value.add(id);
};

const hasImageError = (id: number) => imageErrors.value.has(id);

useSeoMeta({
  title: () => `Inventory Stats`,
});
</script>

<template>
  <UContainer class="w-full max-w-none py-8">
    <div class="flex flex-col gap-6">
      <div class="flex flex-col gap-3">
        <div class="flex flex-col gap-1">
          <p class="text-xs uppercase tracking-[0.2em] text-gray-500 dark:text-gray-400">
            Inventory
          </p>
          <div class="flex flex-wrap items-end justify-between gap-3">
            <div>
              <h1 class="text-2xl font-semibold tracking-tight text-gray-900 dark:text-gray-100">
                Stats overview
              </h1>
              <p class="text-sm text-gray-600 dark:text-gray-300">
                Snapshot of items and cargo quantities in circulation.
              </p>
            </div>
            <div
              class="flex items-center gap-2 rounded-full border border-gray-200 px-3 py-1 text-xs text-gray-600 shadow-sm dark:border-gray-800 dark:text-gray-300"
            >
              <span>Items</span>
              <span class="font-semibold text-gray-900 dark:text-gray-100">
                {{ numberFormat.format(Object.keys(data?.items || {}).length) }}
              </span>
              <span class="text-gray-400">/</span>
              <span>Cargo</span>
              <span class="font-semibold text-gray-900 dark:text-gray-100">
                {{ numberFormat.format(Object.keys(data?.cargo || {}).length) }}
              </span>
            </div>
          </div>
        </div>

        <div class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
          <UInput
            v-model="search"
            icon="i-lucide-search"
            placeholder="Search items or rarity"
            variant="outline"
          />
          <USelect v-model="tag" :items="tagOptions" placeholder="All tags" variant="outline" />
          <USelect v-model="tier" :items="tierOptions" placeholder="All tiers" variant="outline" />
          <USelect
            v-model="rarity"
            :items="rarityOptions"
            placeholder="All rarities"
            variant="outline"
          />
        </div>
      </div>

      <UProgress v-if="pending" color="neutral" />

      <template v-if="data">
        <div class="flex flex-col gap-6">
          <div class="flex items-center justify-between">
            <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100">Items</h2>
            <span class="text-sm text-gray-500 dark:text-gray-400">
              Showing {{ numberFormat.format(currentItems.length) }} of {{ numberFormat.format(items.length) }}
            </span>
          </div>
          <div class="grid gap-2 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
            <UCard
              v-for="{ item, icon } in currentItems"
              :key="`item:${item[1].id}`"
              :class="['border border-l-4', tierToBorderClass(item[1].tier)]"
            >
              <div class="flex flex-col gap-0">
                <div
                  class="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400"
                >
                  <div class="flex items-center gap-1">
                    <span class="uppercase tracking-widest">Qty</span>
                    <span class="text-sm font-semibold text-gray-900 dark:text-gray-100">
                      {{ numberFormat.format(item[0]) }}
                    </span>
                  </div>
                  <span v-if="item[1].tier > 0" :class="tierToTextClass(item[1].tier)">
                    Tier {{ item[1].tier }}
                  </span>
                  <span v-if="item[1].tag">
                    {{ item[1].tag }}
                  </span>
                </div>
                <div class="flex items-center gap-2">
                  <picture v-if="icon && !hasImageError(item[1].id)">
                    <source :srcset="`${icon}.jxl`" type="image/jxl" />
                    <source :srcset="`${icon}.avif`" type="image/avif" />
                    <img
                      :src="`${icon}.webp`"
                      :alt="item[1].name"
                      class="h-9 w-9 rounded border border-gray-200 object-cover dark:border-gray-800"
                      loading="lazy"
                      @error="onImageError(item[1].id)"
                    />
                  </picture>
                  <div class="text-sm font-semibold text-gray-900 dark:text-gray-100">
                    {{ item[1].name }}
                  </div>
                </div>
                <div
                  class="text-xs uppercase tracking-widest"
                  :class="rarityToTextClass(item[1].rarity)"
                >
                  {{ item[1].rarity }}
                </div>
                 <details class="mt-2 text-xs text-gray-600 dark:text-gray-300" @toggle="toggleRegions(`item:${item[1].id}`, $event)">
                   <summary class="cursor-pointer">By region</summary>
                   <template v-if="expandedRegions.has(`item:${item[1].id}`)">
                   <div
                    v-for="[region, quantity] in regionsByQuantity(item[2])"
                    :key="region"
                    class="flex justify-between gap-3"
                  >
                    <span>Region <bitcraft-region :region="Number(region)" /></span>
                    <span>{{ numberFormat.format(quantity) }}</span>
                   </div>
                   </template>
                </details>
              </div>
            </UCard>
          </div>
          <UPagination v-if="items.length > pageSize" v-model:page="itemsPage" :total="items.length" :items-per-page="pageSize" class="self-center" />

          <div class="h-px bg-gray-200 dark:bg-gray-800"></div>

          <div class="flex items-center justify-between">
            <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100">Cargo</h2>
            <span class="text-sm text-gray-500 dark:text-gray-400">
              Showing {{ numberFormat.format(currentCargo.length) }} of {{ numberFormat.format(cargo.length) }}
            </span>
          </div>
          <div class="grid gap-2 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
            <UCard
              v-for="{ item, icon } in currentCargo"
              :key="`cargo:${item[1].id}`"
              :class="['border border-l-4', tierToBorderClass(item[1].tier)]"
            >
              <div class="flex flex-col gap-0">
                <div
                  class="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400"
                >
                  <div class="flex items-center gap-1">
                    <span class="uppercase tracking-widest">Qty</span>
                    <span class="text-sm font-semibold text-gray-900 dark:text-gray-100">
                      {{ numberFormat.format(item[0]) }}
                    </span>
                  </div>
                  <span v-if="item[1].tier > 0" :class="tierToTextClass(item[1].tier)">
                    Tier {{ item[1].tier }}
                  </span>
                  <span v-if="item[1].tag">
                    {{ item[1].tag }}
                  </span>
                </div>
                <div class="flex items-center gap-2">
                  <picture v-if="icon && !hasImageError(item[1].id)">
                    <source :srcset="`${icon}.jxl`" type="image/jxl" />
                    <source :srcset="`${icon}.avif`" type="image/avif" />
                    <img
                      :src="`${icon}.webp`"
                      :alt="item[1].name"
                      class="h-9 w-9 rounded border border-gray-200 object-cover dark:border-gray-800"
                      loading="lazy"
                      @error="onImageError(item[1].id)"
                    />
                  </picture>
                  <div class="text-sm font-semibold text-gray-900 dark:text-gray-100">
                    {{ item[1].name }}
                  </div>
                </div>
                <div
                  class="text-xs uppercase tracking-widest"
                  :class="rarityToTextClass(item[1].rarity)"
                >
                  {{ item[1].rarity }}
                </div>
                 <details class="mt-2 text-xs text-gray-600 dark:text-gray-300" @toggle="toggleRegions(`cargo:${item[1].id}`, $event)">
                   <summary class="cursor-pointer">By region</summary>
                   <template v-if="expandedRegions.has(`cargo:${item[1].id}`)">
                   <div
                    v-for="[region, quantity] in regionsByQuantity(item[2])"
                    :key="region"
                    class="flex justify-between gap-3"
                  >
                    <span>Region <bitcraft-region :region="Number(region)" /></span>
                    <span>{{ numberFormat.format(quantity) }}</span>
                   </div>
                   </template>
                </details>
              </div>
            </UCard>
          </div>
          <UPagination v-if="cargo.length > pageSize" v-model:page="cargoPage" :total="cargo.length" :items-per-page="pageSize" class="self-center" />
        </div>
      </template>
      <template v-else>
        <UEmpty
          icon="i-lucide-package-search"
          title="No inventory stats"
          description="Try changing your search criteria."
        />
      </template>
    </div>
  </UContainer>
</template>
