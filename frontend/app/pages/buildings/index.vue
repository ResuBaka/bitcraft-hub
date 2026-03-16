<script setup lang="ts">
import { watchThrottled } from "@vueuse/shared";
import { useDelayedPending } from "~/utils";
import type { BuildingDescriptionsResponse } from "~/types/BuildingDescriptionsResponse";

const page = ref(1);
const perPage = 30;

const search = ref<string | null>("");

const route = useRoute();
const router = useRouter();

const tmpPage = (route.query.page as string) ?? null;

if (tmpPage) {
  page.value = parseInt(tmpPage);
}
const tmpSearch = (route.query.search as string) ?? null;

if (tmpSearch) {
  search.value = tmpSearch;
}

const { data, pending, refresh } = await useLazyFetchMsPack<BuildingDescriptionsResponse>(
  () => {
    return `/api/bitcraft/desc/buildings`;
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
  () => [search.value],
  (value, oldValue) => {
    if (value[0] !== oldValue[0]) {
      page.value = 1;
    }

    refresh();
  },
  { throttle: 50 },
);

const currentBuildings = computed(() => {
  return data.value?.buildings ?? [];
});

useSeoMeta({
  title: "Buildings",
  description: "List of all buildings in Bitcraft",
});
</script>

<template>
  <UContainer class="w-full max-w-none py-8">
    <div class="flex flex-col gap-6">
      <div class="flex flex-col gap-3">
        <div class="flex flex-col gap-1">
          <p class="text-xs uppercase tracking-[0.2em] text-gray-500 dark:text-gray-400">
            Buildings
          </p>
          <div class="flex flex-wrap items-end justify-between gap-3">
            <div>
              <h1 class="text-2xl font-semibold tracking-tight text-gray-900 dark:text-gray-100">
                Building catalog
              </h1>
              <p class="text-sm text-gray-600 dark:text-gray-300">
                Browse all buildings and inspect their details.
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

        <UInput
          v-model="search"
          icon="i-lucide-search"
          placeholder="Search buildings"
          variant="outline"
          class="max-w-md"
        />
      </div>

      <div class="flex min-h-[44px] justify-center pb-4" :class="showPending ? 'opacity-60' : ''">
        <UPagination
          v-model:page="page"
          :total="Number(data?.total ?? 0)"
          :items-per-page="perPage"
          size="sm"
          :disabled="showPending"
          :sibling-count="4"
          @update:page="changePage"
        />
      </div>

      <UProgress v-if="showPending" color="neutral" />

      <template v-if="currentBuildings.length">
        <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 2xl:grid-cols-5">
          <bitcraft-building-card
            v-for="building in currentBuildings"
            :key="building.id"
            :building="building"
          />
        </div>
      </template>
      <UEmpty
        v-else
        icon="i-lucide-building-2"
        title="No buildings found"
        description="Try adjusting your search criteria."
      />
    </div>
  </UContainer>
</template>
