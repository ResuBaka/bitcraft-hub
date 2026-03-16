<script setup lang="ts">
import type { BuildingState } from "~/types/BuildingState";
import type { InventorysResponse } from "~/types/InventorysResponse";

const route = useRoute();

const { data: buildingsFetch, pending: buildingPending } = useFetchMsPack<BuildingState>(() => {
  return `/buildings/${route.params.id}`;
});

const { data: inventoryFetch, pending: inventoryPending } = useFetchMsPack<InventorysResponse>(
  () => {
    return `/api/bitcraft/inventorys/owner_entity_id/${route.params.id}`;
  },
);

const inventories = computed(() => {
  return inventoryFetch.value?.inventorys ?? [];
});

const buildingTitle = computed(() => {
  const nickname = buildingsFetch.value?.nickname;
  if (nickname && nickname !== "") {
    return nickname;
  }

  return buildingsFetch.value?.entity_id?.toString() || route.params.id.toString();
});

useSeoMeta({
  title: () => `Building ${buildingTitle.value}`,
});
</script>

<template>
  <UContainer class="w-full max-w-none py-8">
    <div class="flex flex-col gap-6">
      <UCard :ui="{ header: 'p-4', body: 'p-4' }">
        <template #header>
          <div class="flex flex-wrap items-end justify-between gap-3">
            <div>
              <p class="text-xs uppercase tracking-[0.2em] text-gray-500 dark:text-gray-400">
                Building
              </p>
              <h1 class="text-2xl font-semibold tracking-tight text-gray-900 dark:text-gray-100">
                {{ buildingTitle }}
              </h1>
            </div>
            <UBadge color="neutral" variant="soft"> ID: {{ route.params.id }} </UBadge>
          </div>
        </template>

        <div v-if="buildingPending || inventoryPending" class="flex justify-center py-4">
          <UIcon name="i-lucide-loader-circle" class="h-6 w-6 animate-spin text-gray-400" />
        </div>

        <template v-else>
          <div class="mb-4 flex items-center justify-between gap-2">
            <h2
              class="text-sm font-semibold uppercase tracking-[0.12em] text-gray-600 dark:text-gray-300"
            >
              Inventories
            </h2>
            <span class="text-xs text-gray-500 dark:text-gray-400">
              {{ inventories.length }} total
            </span>
          </div>

          <div v-if="inventories.length" class="grid grid-cols-1 gap-4 lg:grid-cols-2">
            <bitcraft-inventory
              v-for="inventory in inventories"
              :key="inventory.entity_id.toString()"
              :inventory="inventory"
            />
          </div>

          <UEmpty
            v-else
            icon="i-lucide-box"
            title="No inventories found"
            description="This building does not have visible inventories yet."
          />
        </template>
      </UCard>
    </div>
  </UContainer>
</template>
