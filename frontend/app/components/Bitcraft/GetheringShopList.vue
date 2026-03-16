<script setup lang="ts">
import { iconAssetUrlNameRandom } from "~/composables/iconAssetName";
import type { CargoDesc } from "~/types/CargoDesc";
import type { ItemDesc } from "~/types/ItemDesc";

const props = defineProps<{
  id: number;
  type: "Cargo" | "Item";
  quantity: number;
  item_desc: { [key in number]?: ItemDesc };
  cargo_desc: { [key in number]?: CargoDesc };
}>();
const desc = computed(() => {
  let desc: ItemDesc | CargoDesc | undefined;

  if (props.type === "Item") {
    desc = props.item_desc[props.id];
  } else {
    desc = props.cargo_desc[props.id];
  }
  return desc;
});

const iconUrl = computed(() => {
  const iconName = desc.value?.icon_asset_name;
  if (!iconName) return null;
  const icon = iconAssetUrlNameRandom(iconName);
  return icon.show ? icon.url : null;
});
</script>
<template>
  <div
    v-if="desc"
    class="flex items-center gap-3 rounded-lg border border-gray-200 bg-white/70 p-3 shadow-sm dark:border-gray-800 dark:bg-gray-900/40"
  >
    <div
      class="flex h-12 w-12 items-center justify-center rounded-md border border-gray-200 bg-white dark:border-gray-800 dark:bg-gray-950"
    >
      <img
        v-if="iconUrl"
        :src="iconUrl"
        :alt="desc.name"
        class="h-10 w-10 object-contain"
        loading="lazy"
      />
      <UIcon v-else name="i-lucide-box" class="h-5 w-5 text-gray-400" />
    </div>
    <div class="flex-1">
      <p class="text-sm font-semibold text-gray-900 dark:text-gray-100">
        {{ desc.name }}
      </p>
      <p class="text-xs text-gray-500 dark:text-gray-400">{{ type }} #{{ id }}</p>
    </div>
    <UBadge color="neutral" variant="soft">
      {{ Intl.NumberFormat().format(quantity) }}
    </UBadge>
  </div>
</template>
