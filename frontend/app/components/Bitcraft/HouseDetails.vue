<script setup lang="ts">
import type { HouseInventoriesResponse } from "~/types/HouseInventoriesResponse";
import type { HouseResponse } from "~/types/HouseResponse";
import type { InventoryChangelog } from "~/types/InventoryChangelog";

const props = defineProps<{
  house: HouseResponse;
}>();

const tab = ref<"overview" | "permissions" | "inventories" | "changelog">("overview");

const { data: inventories, pending: invPending } =
  await useLazyFetchMsPack<HouseInventoriesResponse>(
    () => `/api/bitcraft/houses/${props.house.entity_id}/inventories`,
  );

const { data: changelog, pending: changelogPending } = await useLazyFetchMsPack<
  InventoryChangelog[]
>(() => `/api/bitcraft/inventorys/changes/${props.house.entity_id}`);

const inventoryCount = computed(() => inventories.value?.inventories?.length ?? 0);

const occupiedInventories = computed(() => {
  const houseInventories = inventories.value?.inventories ?? [];
  return houseInventories.filter((inventory) =>
    inventory.pockets.some((pocket) => (pocket.contents?.quantity ?? 0) > 0),
  );
});

const changelogCount = computed(() => changelog.value?.length ?? 0);

const permissionColumns = [
  { id: "allowed_username", header: "Username" },
  { id: "group", header: "Group" },
  { id: "rank", header: "Rank" },
];

const getRankName = (rank: number) => {
  switch (rank) {
    case 7:
      return "Owner";
    case 6:
      return "Admin";
    case 5:
      return "Resident";
    case 1:
      return "Guest";
    default:
      return `Rank ${rank}`;
  }
};

const getRankColor = (rank: number) => {
  if (rank >= 7) {
    return "warning";
  }
  if (rank >= 6) {
    return "primary";
  }
  return "neutral";
};
</script>

<template>
  <UCard class="mb-4" :ui="{ header: 'p-4', body: 'p-4' }">
    <template #header>
      <div class="flex flex-wrap items-center gap-3">
        <span class="text-lg font-black">House {{ house.owner_username ?? house.entity_id }}</span>
        <UBadge color="primary" variant="soft">{{ house.region }}</UBadge>
        <UBadge :color="house.is_empty ? 'neutral' : 'success'" variant="soft">
          {{ house.is_empty ? "Empty" : "Occupied" }}
        </UBadge>
      </div>
    </template>

    <div class="flex flex-wrap gap-2">
      <UButton
        color="neutral"
        size="sm"
        :variant="tab === 'overview' ? 'solid' : 'soft'"
        @click="tab = 'overview'"
      >
        Overview
      </UButton>
      <UButton
        color="neutral"
        size="sm"
        :variant="tab === 'permissions' ? 'solid' : 'soft'"
        @click="tab = 'permissions'"
      >
        Permissions ({{ house.permissions.length }})
      </UButton>
      <UButton
        color="neutral"
        size="sm"
        :variant="tab === 'inventories' ? 'solid' : 'soft'"
        @click="tab = 'inventories'"
      >
        Inventories ({{ inventoryCount }})
      </UButton>
      <UButton
        color="neutral"
        size="sm"
        :variant="tab === 'changelog' ? 'solid' : 'soft'"
        @click="tab = 'changelog'"
      >
        Changes ({{ changelogCount }})
      </UButton>
    </div>

    <div class="mt-4 border-t border-gray-200 pt-4 dark:border-gray-800">
      <template v-if="tab === 'overview'">
        <div class="grid gap-3 sm:grid-cols-2">
          <UCard :ui="{ body: 'p-3' }">
            <p class="text-xs uppercase tracking-wide text-gray-500 dark:text-gray-400">Region</p>
            <p class="mt-1 text-sm font-medium text-gray-900 dark:text-gray-100">
              <bitcraft-region :region="house.region_index" />
            </p>
          </UCard>
          <UCard :ui="{ body: 'p-3' }">
            <p class="text-xs uppercase tracking-wide text-gray-500 dark:text-gray-400">Rank</p>
            <p class="mt-1 text-sm font-medium text-gray-900 dark:text-gray-100">
              {{ getRankName(house.rank) }}
            </p>
          </UCard>
        </div>
      </template>

      <template v-else-if="tab === 'permissions'">
        <div
          v-if="house.permissions.length > 0"
          class="rounded-lg border border-gray-200 dark:border-gray-800"
        >
          <UTable :columns="permissionColumns" :data="house.permissions">
            <template #allowed_username-cell="{ row }">
              <NuxtLink
                class="text-primary-500 hover:underline"
                :to="{
                  name: 'players-id',
                  params: { id: row.original.allowed_entity_id.toString() },
                }"
              >
                {{ row.original.allowed_username || "Unknown" }}
              </NuxtLink>
            </template>
            <template #group-cell="{ row }">
              {{ row.original.group }}
            </template>
            <template #rank-cell="{ row }">
              <UBadge :color="getRankColor(row.original.rank)" variant="soft" size="xs">
                {{ getRankName(row.original.rank) }}
              </UBadge>
            </template>
          </UTable>
        </div>
        <p v-else class="py-4 text-center text-sm text-gray-500 dark:text-gray-400">
          No explicit permissions found.
        </p>
      </template>

      <template v-else-if="tab === 'inventories'">
        <div v-if="invPending" class="flex justify-center py-4">
          <UIcon name="i-lucide-loader-circle" class="h-6 w-6 animate-spin text-gray-400" />
        </div>
        <div
          v-else-if="occupiedInventories.length > 0"
          class="grid grid-cols-1 gap-4 lg:grid-cols-2"
        >
          <bitcraft-inventory
            v-for="inv in occupiedInventories"
            :key="inv.entity_id.toString()"
            :inventory="inv"
          />
        </div>
        <p v-else class="py-4 text-center text-sm text-gray-500 dark:text-gray-400">
          No interior inventories found.
        </p>
      </template>

      <template v-else-if="tab === 'changelog'">
        <div v-if="changelogPending" class="flex justify-center py-4">
          <UIcon name="i-lucide-loader-circle" class="h-6 w-6 animate-spin text-gray-400" />
        </div>
        <template v-else-if="changelog && changelog.length > 0">
          <bitcraft-inventory-changes :items="changelog" />
        </template>
        <p v-else class="py-4 text-center text-sm text-gray-500 dark:text-gray-400">
          No inventory changes found.
        </p>
      </template>
    </div>
  </UCard>
</template>
