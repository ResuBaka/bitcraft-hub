<script setup lang="ts">
import { useTierColor } from "~/composables/useTierColor";
import type { HouseResponse } from "~/types/HouseResponse";

const props = defineProps<{
  house: HouseResponse;
}>();

const tierColors = useTierColor();

const rankColorClass = computed(() => {
  return tierColors[props.house.rank as keyof typeof tierColors] || "text-gray-500";
});
</script>

<template>
  <UCard class="mb-4" :ui="{ header: 'p-4', body: 'p-4', footer: 'px-4 pb-4 pt-0' }">
    <template #header>
      <div class="flex items-start gap-3">
        <div
          class="flex h-12 w-12 shrink-0 items-center justify-center rounded-xl border border-gray-200 bg-white dark:border-gray-800 dark:bg-gray-900"
        >
          <UIcon name="i-lucide-house" class="h-6 w-6" :class="rankColorClass" />
        </div>

        <div class="min-w-0">
          <NuxtLink
            class="text-base font-black text-gray-900 hover:underline dark:text-gray-100"
            :to="{ name: 'players-id', params: { id: house.owner_entity_id.toString() } }"
          >
            {{ house.owner_username || "Unknown Owner" }}'s House
          </NuxtLink>
          <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">
            Rank {{ house.rank }} | Region {{ house.region }}
          </p>
        </div>
      </div>
    </template>

    <div class="grid gap-2">
      <div class="flex items-center justify-between gap-2 text-sm">
        <span class="font-semibold text-gray-700 dark:text-gray-300">Entity ID</span>
        <span class="font-mono text-xs text-gray-600 dark:text-gray-400">{{
          house.entity_id.toString()
        }}</span>
      </div>
      <div class="flex items-center justify-between gap-2 text-sm">
        <span class="font-semibold text-gray-700 dark:text-gray-300">Entrance Building</span>
        <span class="font-mono text-xs text-gray-600 dark:text-gray-400">
          {{ house.entrance_building_entity_id.toString() }}
        </span>
      </div>
      <div class="flex items-center justify-between gap-2 text-sm">
        <span class="font-semibold text-gray-700 dark:text-gray-300">Status</span>
        <UBadge :color="house.is_empty ? 'neutral' : 'success'" variant="soft" size="xs">
          {{ house.is_empty ? "Empty" : "Occupied" }}
        </UBadge>
      </div>
    </div>

    <template #footer>
      <div class="flex justify-end">
        <UButton
          color="primary"
          variant="soft"
          :to="{ name: 'players-id', params: { id: house.owner_entity_id.toString() } }"
        >
          View Details
        </UButton>
      </div>
    </template>
  </UCard>
</template>
