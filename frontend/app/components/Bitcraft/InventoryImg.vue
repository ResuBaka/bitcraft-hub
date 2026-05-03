<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    item: any;
    skipErrorText?: boolean;
    height?: number | string;
    width?: number | string;
  }>(),
  {
    skipErrorText: false,
    height: 80,
    width: 80,
  },
);

const imagedErrored = ref(false);
const iconUrl = (item: any) => {
  if (!item?.icon_asset_name) {
    return {
      url: "",
      show: false,
    };
  }

  return iconAssetUrlNameRandom(item.icon_asset_name);
};

const stippedName = computed<string>(() => {
  return props.item?.name?.replace(/[[(){}\]]/gi, "") ?? "";
});

const fallbackText = computed(() => {
  if (!stippedName.value) {
    return "";
  }
  return stippedName.value
    .split(" ")
    .map((part) => part.charAt(0))
    .join("")
    .slice(0, 3)
    .toUpperCase();
});

const sizeValue = (value: number | string) => {
  if (typeof value === "number") {
    return `${value}px`;
  }
  return value;
};
</script>

<template>
  <div class="inventory-img" v-bind="$attrs">
    <img
      v-if="!imagedErrored && iconUrl(item).show"
      :src="iconUrl(item).url"
      :alt="item?.name ?? 'Item icon'"
      :style="{
        maxWidth: sizeValue(width),
        maxHeight: sizeValue(height),
        width: 'auto',
        height: 'auto',
      }"
      class="inventory-img__image"
      loading="lazy"
      @error="imagedErrored = true"
    />
    <div
      v-else-if="!skipErrorText && fallbackText"
      class="inventory-img__fallback"
      :style="{ width: sizeValue(width), height: sizeValue(height) }"
    >
      {{ fallbackText }}
    </div>
    <div
      v-else
      class="inventory-img__placeholder"
      :style="{ width: sizeValue(width), height: sizeValue(height) }"
    >
      <UIcon name="i-lucide-box" class="inventory-img__icon" />
    </div>
  </div>
</template>

<style scoped>
.inventory-img {
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.inventory-img__image {
  display: block;
  object-fit: contain;
  filter: drop-shadow(0 4px 8px rgba(15, 23, 42, 0.35));
}

.inventory-img__fallback,
.inventory-img__placeholder {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: rgba(15, 23, 42, 0.35);
  color: rgba(226, 232, 240, 0.9);
  font-size: 0.75rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.08em;
}

.inventory-img__icon {
  width: 22px;
  height: 22px;
  color: rgba(148, 163, 184, 0.7);
}
</style>
