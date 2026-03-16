<script setup lang="ts">
import type { IconAssetUrl } from "~/composables/iconAssetName";

const props = defineProps<{
  item: any;
}>();

const iconUrl = computed<IconAssetUrl>(() => {
  if (!props.item.item.icon_asset_name) {
    return {
      url: "",
      show: false,
    };
  }

  return iconAssetUrlNameAmount(props.item.item.icon_asset_name, props.item.quantity);
});

const iconLoadError = ref(false);

watch(iconUrl, () => {
  iconLoadError.value = false;
});

const showIcon = computed(() => {
  return iconUrl.value.show && !!iconUrl.value.url && !iconLoadError.value;
});

const handleIconError = () => {
  iconLoadError.value = true;
};
</script>

<template>
  <UCard :ui="{ body: 'p-3 sm:p-4' }">
    <div class="flex items-center gap-3">
      <div
        class="relative flex h-12 w-12 items-center justify-center rounded-md border border-gray-200 bg-white dark:border-gray-800 dark:bg-gray-950"
      >
        <img
          v-if="showIcon"
          :src="iconUrl.url"
          :alt="item.item.name"
          class="h-10 w-10 object-contain"
          loading="lazy"
          @error="handleIconError"
        />
        <UIcon v-else name="i-lucide-box" class="h-5 w-5 text-gray-400" />

        <UBadge
          v-if="iconUrl.amount && iconUrl.amount > 1"
          color="neutral"
          variant="solid"
          size="xs"
          class="absolute -right-1 -top-1"
        >
          {{ iconUrl.amount }}
        </UBadge>
      </div>

      <div class="min-w-0 flex-1">
        <p class="truncate text-sm font-medium text-gray-900 dark:text-gray-100">
          {{ item.item.name }}
        </p>
        <p class="text-xs text-gray-500 dark:text-gray-400">Type: Item</p>
      </div>

      <UBadge color="neutral" variant="soft">{{ item.quantity }}</UBadge>
    </div>
  </UCard>
</template>
