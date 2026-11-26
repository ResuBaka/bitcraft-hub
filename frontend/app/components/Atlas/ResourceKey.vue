<script setup lang="ts">
const { color, iconUrl } = defineProps<{ color: string; iconUrl?: string }>();
const loadedUrl = ref<string>();
</script>

<template>
  <span
    class="atlas-resource-key"
    :class="{ 'has-icon': iconUrl && loadedUrl === iconUrl }"
    :style="{ '--resource-color': color }"
    aria-hidden="true"
  >
    <img
      v-if="iconUrl"
      :key="iconUrl"
      :src="iconUrl"
      width="16"
      height="16"
      alt=""
      loading="lazy"
      decoding="async"
      @load="loadedUrl = iconUrl"
      @error="loadedUrl = undefined"
    />
  </span>
</template>

<style scoped>
.atlas-resource-key {
  display: inline-grid;
  width: 1.15rem;
  height: 1.15rem;
  flex: none;
  place-items: center;
  border-radius: 50%;
  position: relative;
}
.atlas-resource-key::before {
  content: "";
  position: absolute;
  width: 0.55rem;
  height: 0.55rem;
  border-radius: 50%;
  background: var(--resource-color);
}
.atlas-resource-key img {
  object-fit: contain;
  visibility: hidden;
  max-width: 100%;
  max-height: 100%;
}
.atlas-resource-key.has-icon::before {
  display: none;
}
.atlas-resource-key.has-icon img {
  visibility: visible;
  filter: brightness(1.3) drop-shadow(0 0.65px 1px rgb(12 19 25 / 65%));
}
</style>
