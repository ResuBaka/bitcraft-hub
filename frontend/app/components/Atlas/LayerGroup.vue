<script setup lang="ts">
import { useMediaQuery } from "@vueuse/core";

const { label, disabled = false } = defineProps<{
  label: string;
  disabled?: boolean;
}>();
const enabled = defineModel<boolean>({ required: true });
const open = defineModel<boolean>("open", { required: true });
const roomOnRight = useMediaQuery("(min-width: 36rem)");
</script>

<template>
  <div class="atlas-layer-group">
    <input v-model="enabled" type="checkbox" :aria-label="`Show ${label}`" :disabled="disabled" />
    <UPopover
      v-model:open="open"
      :content="{
        side: roomOnRight ? 'right' : 'bottom',
        align: 'start',
        sideOffset: roomOnRight ? 20 : 12,
        collisionPadding: 12,
        'aria-label': `${label} options`,
      }"
      :ui="{ content: 'atlas-layer-flyout' }"
    >
      <button
        type="button"
        class="atlas-layer-group-trigger"
        :aria-label="`${label} options`"
        :disabled="disabled"
      >
        {{ label }}
        <UIcon
          :name="roomOnRight ? 'i-lucide-chevron-right' : 'i-lucide-chevron-down'"
          aria-hidden="true"
        />
      </button>
      <template #content>
        <div class="atlas-layer-group-content">
          <h2>{{ label }} options</h2>
          <p v-if="!enabled">This layer is off. Your options are still saved.</p>
          <slot />
        </div>
      </template>
    </UPopover>
  </div>
</template>

<style scoped>
.atlas-layer-group {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  font-size: 0.85rem;
}
.atlas-layer-group input {
  appearance: auto;
  flex: 0 0 1rem;
  width: 1rem;
  height: 1rem;
  accent-color: #f0c78c;
  cursor: pointer;
}
.atlas-layer-group-trigger {
  display: flex;
  flex: 1;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  padding: 0.3rem 0.35rem 0.3rem 0.25rem;
  border-radius: 0.35rem;
  text-align: left;
  cursor: pointer;
}
.atlas-layer-group-trigger :deep(svg),
.atlas-layer-group-trigger :deep([data-icon]) {
  color: #f0c78c;
}
.atlas-layer-group-trigger:hover:not(:disabled),
.atlas-layer-group-trigger[aria-expanded="true"] {
  background: #274854;
}
.atlas-layer-group :disabled {
  opacity: 0.55;
  cursor: default;
}
.atlas-layer-group :focus-visible {
  outline: 2px solid #f0c78c;
  outline-offset: 1px;
}
:global(.atlas-layer-flyout) {
  box-sizing: border-box;
  width: min(16rem, calc(100vw - 1.5rem));
  max-height: min(32rem, var(--reka-popper-available-height, 62dvh));
  overflow: auto;
  overscroll-behavior: contain;
  border: 1px solid #47616b;
  border-radius: 0.55rem;
  background: #132b37;
  color: #e0edf0;
  box-shadow: 0 6px 18px rgb(0 0 0 / 30%);
}
.atlas-layer-group-content {
  padding: 0.8rem;
}
.atlas-layer-group-content h2 {
  font-size: 0.9rem;
  font-weight: 600;
}
.atlas-layer-group-content p {
  margin-top: 0.35rem;
  color: #a9c0c9;
  font-size: 0.75rem;
}
</style>
