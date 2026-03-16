<script setup lang="ts">
import { iconAssetUrlNameRandom } from "~/composables/iconAssetName";
import type { BuildingDescRow } from "~/modules/bitcraft/gamestate/buildingDesc";

const imageErrored = ref(false);

const { building } = defineProps<{
  building: BuildingDescRow;
}>();

const iconUrl = computed(() => {
  if (!building.icon_asset_name) {
    return {
      url: "",
      show: false,
    };
  }

  return iconAssetUrlNameRandom(building.icon_asset_name);
});

const tier = computed(() => building.functions[0]?.level ?? "-");
</script>

<template>
  <UCard :ui="{ header: 'p-4', body: 'p-4' }">
    <template #header>
      <div class="flex items-start gap-3">
        <div
          class="flex h-14 w-14 shrink-0 items-center justify-center rounded-md border border-gray-200 bg-white dark:border-gray-800 dark:bg-gray-950"
        >
          <img
            v-if="iconUrl.show && imageErrored !== true"
            :src="iconUrl.url"
            :alt="building.name"
            class="h-13 w-13 object-contain"
            loading="lazy"
            @error="imageErrored = true"
          />
          <UIcon v-else name="i-lucide-hammer" class="h-6 w-6 text-gray-400" />
        </div>

        <div class="min-w-0">
          <NuxtLink
            class="text-base font-black text-gray-900 hover:underline dark:text-gray-100"
            :to="{ name: 'buildings-id', params: { id: building.id } }"
          >
            {{ building.name }} ({{ building.count }})
          </NuxtLink>
        </div>
      </div>
    </template>

    <div class="grid gap-2 text-sm">
      <div class="flex items-center justify-between gap-3">
        <span class="font-semibold text-gray-700 dark:text-gray-300">Tier</span>
        <span class="text-gray-600 dark:text-gray-400">{{ tier }}</span>
      </div>
      <div class="flex items-start justify-between gap-3">
        <span class="font-semibold text-gray-700 dark:text-gray-300">Description</span>
        <span class="max-w-[70%] text-right text-gray-600 dark:text-gray-400">
          {{ building.description || "-" }}
        </span>
      </div>
    </div>
  </UCard>
</template>

<style scoped></style>
