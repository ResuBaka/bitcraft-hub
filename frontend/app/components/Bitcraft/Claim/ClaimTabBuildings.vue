<script setup lang="ts">
import type { BuildingStateWithName } from "~/types/BuildingStateWithName";

defineProps<{
  search: string | null;
  buildings: BuildingStateWithName[];
  buildingsPending: boolean;
  page: number;
  total: number;
  perPage: number;
  iconDomain?: string;
  getBuildingIcon: (buildingDescId: number) => string | undefined;
}>();

const emit = defineEmits<{
  "update:search": [value: string | null];
  "update:page": [value: number];
}>();
</script>

<template>
  <div class="flex flex-col gap-3">
    <div class="flex flex-wrap items-center gap-2">
      <UInput
        :model-value="search"
        icon="i-heroicons-magnifying-glass"
        placeholder="Search buildings"
        class="w-full sm:w-64"
        @update:model-value="(value) => emit('update:search', value)"
      />
    </div>
    <UProgress v-if="buildingsPending" color="neutral" />
    <div class="grid gap-2 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
      <NuxtLink
        v-for="building in buildings"
        :key="building.entity_id"
        :to="{ name: 'buildings-id', params: { id: building.entity_id.toString() } }"
        class="flex items-center gap-3 rounded-lg border border-gray-200 p-3 text-sm font-semibold text-gray-900 transition hover:border-gray-300 hover:bg-gray-50 dark:border-gray-800 dark:text-gray-100 dark:hover:bg-gray-900"
      >
        <img
          v-if="iconDomain"
          :src="`${iconDomain}/${getBuildingIcon(building.building_description_id)}.webp`"
          alt=""
          class="h-10 w-10 rounded-md object-cover"
        />
        <span>{{ building.building_name }}</span>
      </NuxtLink>
    </div>
    <div class="flex justify-center">
      <UPagination
        :page="page"
        :total="total"
        :items-per-page="perPage"
        @update:page="(value) => emit('update:page', value)"
      />
    </div>
  </div>
</template>
