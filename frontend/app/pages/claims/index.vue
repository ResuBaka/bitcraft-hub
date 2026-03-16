<script setup lang="ts">
import { watchThrottled } from "@vueuse/shared";
import { useDelayedPending } from "~/utils";

const page = ref(1);
const perPage = 20;

const search = ref<string | null>("");

const route = useRoute();
const router = useRouter();

const showEmptySupplies = ref(false);

if (route.query.search) {
  search.value = route.query.search as string;
}

if (route.query.page) {
  page.value = parseInt(route.query.page);
}

const {
  data: claims,
  pending,
  refresh,
} = await useLazyFetchMsPack(
  () => {
    return `/api/bitcraft/claims`;
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

      if (showEmptySupplies.value) {
        options.query.ses = showEmptySupplies.value.toString();
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
  () => [search.value, showEmptySupplies.value],
  (value, oldValue) => {
    if (value[0] !== oldValue[0] || value[1] !== oldValue[1]) {
      page.value = 1;
    }

    refresh();
  },
  { throttle: 50 },
);

const currentClaims = computed(() => {
  return claims.value?.claims ?? [];
});

const length = computed(() => {
  if (claims.value?.total) {
    return Math.ceil(claims.value?.total / perPage);
  }

  return 0;
});

useSeoMeta({
  title: "Claims",
  description: "List of all the Claims in the game",
});
</script>

<template>
  <UContainer class="py-6 max-w-none">
    <div class="flex flex-col gap-4">
      <div class="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
        <div>
          <h1 class="text-xl font-semibold text-gray-900 dark:text-gray-100">Claims</h1>
          <p class="text-sm text-gray-500 dark:text-gray-400">List of all the Claims in the game</p>
        </div>
        <div class="flex flex-col gap-3 sm:flex-row sm:items-center">
          <UInput
            v-model="search"
            placeholder="Search claims"
            icon="i-heroicons-magnifying-glass"
            class="w-full sm:w-64"
          />
          <UCheckbox v-model="showEmptySupplies" label="Show empty supplies" />
        </div>
      </div>

      <div class="flex flex-col gap-3">
        <div class="flex justify-center">
          <UPagination
            v-model:page="page"
            :total="claims?.total ?? 0"
            :items-per-page="perPage"
            :disabled="showPending"
            :sibling-count="4"
            @update:page="changePage"
          />
        </div>
        <UProgress v-if="showPending" animation="carousel" />
      </div>

      <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 2xl:grid-cols-5">
        <bitcraft-card-claim v-for="claim in currentClaims" :key="claim.entity_id" :claim="claim" />
      </div>
    </div>
  </UContainer>
</template>
