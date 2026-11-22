<script setup lang="ts">
import { useDelayedPending } from "~/utils";
import type { ClaimResponse } from "~/types/ClaimResponse";

const perPage = 20;
const { page, search, searchTerm, filter } = useCatalogControls();
const showEmptySupplies = filter("ses", (value) => value === "true", (value) => value ? "true" : undefined);
const { data: claims, pending } = await useLazyFetchMsPack<ClaimResponse>(
  "/api/bitcraft/claims",
  {
    key: "catalog:claims",
    query: computed(() => ({
      page: page.value,
      per_page: perPage,
      search: searchTerm.value,
      ses: showEmptySupplies.value || undefined,
    })),
  },
);

const showPending = useDelayedPending(pending, 150);

const currentClaims = computed(() => {
  return claims.value?.claims ?? [];
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
