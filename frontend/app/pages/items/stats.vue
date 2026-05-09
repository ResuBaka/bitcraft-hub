<script setup lang="ts">
import { iconAssetUrlNameRandom } from "~/composables/iconAssetName";
import { rarityToTextClass, tierToBorderClass, tierToTextClass } from "~/utils";
import type { AllInventoryStatsResponse } from "~/types/AllInventoryStatsResponse";
import type { MetaResponse } from "~/types/MetaResponse";
import type { ItemExpended } from "~/types/ItemExpended";

const page = ref(1);

const tag = ref<string | null>(null);
const tier = ref<number | null>(null);
const search = ref<string | null>("");
const debouncedSearch = ref<string | null>("");

const route = useRoute();

debouncedSearch.value = (route.query.search as string) ?? "";
search.value = debouncedSearch.value;
tag.value = (route.query.tag as string) ?? null;

if (route.query.tier) {
  tier.value = parseInt(route.query.tier);
}

const { data, pending } = await useLazyFetchMsPack<AllInventoryStatsResponse>(() => {
  return `/inventory/all_inventory_stats`;
});

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

const items = computed(() => {
  if (!data || !data?.value) {
    return [];
  }
  const filtered = Object.values(data.value.items).filter((value) => {
    if (tag.value && value[1]?.tag !== tag.value) {
      return false;
    }

    if (tier.value && value[1]?.tier !== tier.value) {
      return false;
    }

    if (!debouncedSearch.value) {
      return true;
    }

    if (value[1].name.toLocaleLowerCase().includes(debouncedSearch.value?.toLocaleLowerCase())) {
      return true;
    }

    if (value[1].rarity.toLocaleLowerCase().includes(debouncedSearch.value?.toLocaleLowerCase())) {
      return true;
    }

    return false;
  });

  if (!debouncedSearch.value && !tag.value && !tier.value) {
    return filtered.slice(0, 19);
  }

  return filtered;
});

const cargo = computed(() => {
  if (!data || !data?.value) {
    return [];
  }
  const filtered = Object.values(data.value.cargo).filter((value) => {
    if (tag.value && value[1]?.tag !== tag.value) {
      return false;
    }

    if (tier.value && value[1]?.tier !== tier.value) {
      return false;
    }

    if (!debouncedSearch.value) {
      return true;
    }

    if (value[1].name.toLocaleLowerCase().includes(debouncedSearch.value?.toLocaleLowerCase())) {
      return true;
    }

    if (value[1].rarity.toLocaleLowerCase().includes(debouncedSearch.value?.toLocaleLowerCase())) {
      return true;
    }

    return false;
  });

  if (!debouncedSearch.value && !tag.value && !tier.value) {
    return filtered.slice(0, 19);
  }

  return filtered;
});

const numberFormat = new Intl.NumberFormat();

const imageErrors = ref(new Set<number>());

const iconForItem = (item: ItemExpended) => {
  if (!item?.icon_asset_name) {
    return null;
  }

  const icon = iconAssetUrlNameRandom(item.icon_asset_name);
  return icon.show ? icon.url : null;
};

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

        <div class="grid grid-cols-1 gap-3 sm:grid-cols-3">
          <UInput
            v-model="debouncedSearch"
            icon="i-lucide-search"
            placeholder="Search items or rarity"
            variant="outline"
          />
          <USelect v-model="tag" :items="tagOptions" placeholder="All tags" variant="outline" />
          <USelect v-model="tier" :items="tierOptions" placeholder="All tiers" variant="outline" />
        </div>
      </div>

      <UProgress v-if="pending" color="neutral" />

      <template v-if="data">
        <div class="flex flex-col gap-6">
          <div class="flex items-center justify-between">
            <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100">Items</h2>
            <span class="text-sm text-gray-500 dark:text-gray-400">
              Showing {{ numberFormat.format(items.length) }}
            </span>
          </div>
          <div class="grid gap-2 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
            <UCard
              v-for="item in items"
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
                </div>
                <div class="flex items-center gap-2">
                  <img
                    v-if="iconForItem(item[1]) && !hasImageError(item[1].id)"
                    :src="iconForItem(item[1])"
                    :alt="item[1].name"
                    class="h-9 w-9 rounded border border-gray-200 object-cover dark:border-gray-800"
                    loading="lazy"
                    @error="onImageError(item[1].id)"
                  />
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
              </div>
            </UCard>
          </div>

          <div class="h-px bg-gray-200 dark:bg-gray-800"></div>

          <div class="flex items-center justify-between">
            <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100">Cargo</h2>
            <span class="text-sm text-gray-500 dark:text-gray-400">
              Showing {{ numberFormat.format(cargo.length) }}
            </span>
          </div>
          <div class="grid gap-2 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
            <UCard
              v-for="item in cargo"
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
                </div>
                <div class="flex items-center gap-2">
                  <img
                    v-if="iconForItem(item[1]) && !hasImageError(item[1].id)"
                    :src="iconForItem(item[1])"
                    :alt="item[1].name"
                    class="h-9 w-9 rounded border border-gray-200 object-cover dark:border-gray-800"
                    loading="lazy"
                    @error="onImageError(item[1].id)"
                  />
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
              </div>
            </UCard>
          </div>
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
