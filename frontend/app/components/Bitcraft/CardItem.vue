<script setup lang="ts">
import { iconAssetUrlNameRandom } from "~/composables/iconAssetName";
import type { ItemRow } from "~/modules/bitcraft/gamestate/item";

const imageErrored = ref(false);

const dev = import.meta.dev;

const { item } = defineProps<{
  item: ItemRow;
}>();

const {
  data: neededInCrafting,
  execute: neededInCraftingExecute,
  status: neededInCraftingStatus,
} = await useLazyFetchMsPack(
  () => {
    return `/api/bitcraft/recipes/needed_in_crafting/${item.id}`;
  },
  {
    immediate: false,
  },
);

const {
  data: producedInCrafting,
  execute: producedInCraftingExecute,
  status: producedInCraftingStatus,
} = await useLazyFetchMsPack(
  () => {
    return `/api/bitcraft/recipes/produced_in_crafting/${item.id}`;
  },
  {
    immediate: false,
  },
);

const {
  data: neededToCraft,
  execute: neededToCraftExecute,
  status: neededToCraftStatus,
} = await useLazyFetchMsPack(
  () => {
    return `/api/bitcraft/recipes/needed_to_craft/${item.id}`;
  },
  {
    immediate: false,
  },
);

const neededInCraftingData = computed(() => {
  return neededInCrafting.value ?? [];
});

const producedInCraftingData = computed(() => {
  return producedInCrafting.value ?? [];
});

const neededToCraftData = computed(() => {
  return neededToCraft.value ?? [];
});

const contentToShow = ref<
  "default" | "neededToCraft" | "neededInCrafting" | "producedInCrafting"
>("default");

const toggleContentToShow = (
  contentArg:
    | "default"
    | "neededToCraft"
    | "neededInCrafting"
    | "producedInCrafting",
) => {
  if (contentArg === "neededToCraft") {
    if (
      neededToCraftData.value.length === 0 &&
      neededToCraftStatus.value !== "success"
    ) {
      neededToCraftExecute();
    }
  }

  if (contentArg === "neededInCrafting") {
    if (
      neededInCraftingData.value.length === 0 &&
      neededInCraftingStatus.value !== "success"
    ) {
      neededInCraftingExecute();
    }
  }

  if (contentArg === "producedInCrafting") {
    if (
      producedInCraftingData.value.length === 0 &&
      producedInCraftingStatus.value !== "success"
    ) {
      producedInCraftingExecute();
    }
  }

  contentToShow.value = contentArg;
};

const iconUrl = computed(() => {
  if (!item.icon_asset_name) {
    return {
      url: "",
      show: false,
    };
  }

  return iconAssetUrlNameRandom(item.icon_asset_name);
});

const tierColorClass = computed(() => `color-tier-${item.tier}`);

const isLoadingSection = computed(() => {
  if (contentToShow.value === "neededToCraft")
    return neededToCraftStatus.value === "pending";
  if (contentToShow.value === "neededInCrafting")
    return neededInCraftingStatus.value === "pending";
  if (contentToShow.value === "producedInCrafting")
    return producedInCraftingStatus.value === "pending";
  return false;
});
</script>

<template>
  <UCard class="h-full" :ui="{ header: 'p-4', body: 'p-4' }">
    <template #header>
      <div class="flex items-start justify-between gap-3">
        <div class="flex min-w-0 items-start gap-3">
          <div
            class="flex h-12 w-12 shrink-0 items-center justify-center rounded-md border border-gray-200 bg-white dark:border-gray-800 dark:bg-gray-950"
          >
            <img
              v-if="iconUrl.show && imageErrored !== true"
              :src="iconUrl.url"
              :alt="item.name"
              class="h-10 w-10 object-contain"
              loading="lazy"
              @error="imageErrored = true"
            />
            <UIcon v-else name="i-lucide-box" class="h-6 w-6 text-gray-400" />
          </div>

          <div class="min-w-0">
            <NuxtLink
              :class="['truncate text-base font-bold hover:underline', tierColorClass]"
              :to="{ name: 'items-type-id', params: { id: item.id, type: item.type } }"
            >
              {{ item.name }}
            </NuxtLink>
            <p :class="['text-xs', tierColorClass]">
              <template v-if="dev">Id: {{ item.id }} | </template>
              Tier: {{ item.tier }} | Tag: {{ item.tag }} | Rarity: {{ item.rarity }}
            </p>
          </div>
        </div>

        <div class="flex shrink-0 items-center gap-1">
          <UTooltip text="Overview">
            <UButton
              color="neutral"
              variant="ghost"
              icon="i-lucide-house"
              size="sm"
              @click="toggleContentToShow('default')"
            />
          </UTooltip>
          <UTooltip text="Needed to craft">
            <UButton
              color="neutral"
              variant="ghost"
              icon="i-lucide-layers"
              size="sm"
              @click="toggleContentToShow('neededToCraft')"
            />
          </UTooltip>
          <UTooltip text="Needed in crafting">
            <UButton color="neutral" variant="ghost" size="sm" @click="toggleContentToShow('neededInCrafting')">
              <UBadge color="primary" variant="soft" size="xs">
                {{ neededInCraftingData.length }}
              </UBadge>
              <UIcon name="i-lucide-git-fork" class="ml-1 h-4 w-4" />
            </UButton>
          </UTooltip>
          <UTooltip text="Produced in crafting">
            <UButton
              color="neutral"
              variant="ghost"
              size="sm"
              @click="toggleContentToShow('producedInCrafting')"
            >
              <UBadge color="primary" variant="soft" size="xs">
                {{ producedInCraftingData.length }}
              </UBadge>
              <UIcon name="i-lucide-git-branch" class="ml-1 h-4 w-4" />
            </UButton>
          </UTooltip>
        </div>
      </div>
    </template>

    <div v-if="isLoadingSection" class="flex justify-center py-6">
      <UIcon name="i-lucide-loader-circle" class="h-6 w-6 animate-spin text-gray-400" />
    </div>

    <template v-else-if="contentToShow === 'default'">
      <div class="grid gap-2 text-sm">
        <div class="flex items-start justify-between gap-3">
          <span class="font-semibold text-gray-700 dark:text-gray-300">Description</span>
          <span class="max-w-[70%] text-right text-gray-600 dark:text-gray-400">
            {{ item.description || "-" }}
          </span>
        </div>
        <div class="flex items-center justify-between gap-3">
          <span class="font-semibold text-gray-700 dark:text-gray-300">Volume</span>
          <span class="text-gray-600 dark:text-gray-400">{{ item.volume }}</span>
        </div>
        <div class="flex items-center justify-between gap-3">
          <span class="font-semibold text-gray-700 dark:text-gray-300">Effort</span>
          <span class="text-gray-600 dark:text-gray-400">
            {{ producedInCraftingData.length ? producedInCraftingData[0].actions_required : 0 }}
          </span>
        </div>
      </div>
    </template>

    <template v-else-if="contentToShow === 'neededInCrafting'">
      <div v-if="neededInCraftingData.length" class="flex flex-col gap-2">
        <UCard
          v-for="crafting in neededInCraftingData"
          :key="crafting.id"
          :ui="{ body: 'p-3' }"
        >
          <p class="text-sm font-semibold text-gray-900 dark:text-gray-100">
            <bitcraft-card-item-crafting-name
              :item="item"
              :template="crafting.name"
              :craftId="crafting.crafted_item_stacks[0].item_id"
            />
          </p>
          <p class="text-xs text-gray-500 dark:text-gray-400">
            time_requirement: {{ crafting.time_requirement }}
          </p>
          <p class="text-xs text-gray-500 dark:text-gray-400">
            stamina_requirement: {{ crafting.stamina_requirement }}
          </p>
          <p class="text-xs text-gray-500 dark:text-gray-400">
            Experience: {{ crafting.completion_experience[0].quantity }}
          </p>
        </UCard>
      </div>
      <p v-else class="py-4 text-center text-sm text-gray-500 dark:text-gray-400">
        No matching crafting recipes.
      </p>
    </template>

    <template v-else-if="contentToShow === 'neededToCraft'">
      <bitcraft-item-stack v-if="neededToCraftData.length" :items="neededToCraftData" />
      <p v-else class="py-4 text-center text-sm text-gray-500 dark:text-gray-400">
        No required crafting stack found.
      </p>
    </template>

    <template v-else-if="contentToShow === 'producedInCrafting'">
      <div v-if="producedInCraftingData.length" class="flex flex-col gap-2">
        <UCard
          v-for="crafting in producedInCraftingData"
          :key="crafting.id"
          :ui="{ body: 'p-3' }"
        >
          <p class="text-sm font-semibold text-gray-900 dark:text-gray-100">
            <bitcraft-card-item-crafting-name
              :item="item"
              :template="crafting.name"
              :craftId="crafting.crafted_item_stacks[0].item_id"
            />
          </p>
          <p class="text-xs text-gray-500 dark:text-gray-400">
            time_requirement: {{ crafting.time_requirement }}
          </p>
          <p class="text-xs text-gray-500 dark:text-gray-400">
            stamina_requirement: {{ crafting.stamina_requirement }}
          </p>
          <p class="text-xs text-gray-500 dark:text-gray-400">
            Experience: {{ crafting.completion_experience[0].quantity }}
          </p>
        </UCard>
      </div>
      <p v-else class="py-4 text-center text-sm text-gray-500 dark:text-gray-400">
        No produced crafting recipes found.
      </p>
    </template>
</template>

<style scoped></style>
