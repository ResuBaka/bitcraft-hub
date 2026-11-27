<script setup lang="ts">
import { normalizeAtlasResourceColor } from "~/utils/atlasResources";

const props = withDefaults(
  defineProps<{ name: string; color: string; custom: boolean; description?: string }>(),
  { description: "Used for dots and cluster badges. In-game icons keep their original colors." },
);
const emit = defineEmits<{ change: [color: string | undefined] }>();
const open = ref(false);
const hex = ref(props.color);
const invalid = ref(false);

watch([open, () => props.color], () => {
  hex.value = props.color;
  invalid.value = false;
});

function apply(value: string | undefined) {
  const color = normalizeAtlasResourceColor(value);
  invalid.value = !color;
  if (!color) return;
  hex.value = color;
  emit("change", color);
}

function reset() {
  invalid.value = false;
  emit("change", undefined);
}
</script>

<template>
  <UPopover
    v-model:open="open"
    :content="{ side: 'right', align: 'start', 'aria-label': `Color for ${name}` }"
    :ui="{ content: 'atlas-resource-color-flyout' }"
  >
    <button
      type="button"
      class="atlas-resource-swatch"
      :aria-label="`Change color for ${name}`"
      :title="`Change color for ${name}`"
    >
      <span :style="{ backgroundColor: color }"></span>
    </button>
    <template #content>
      <div class="atlas-resource-color-content">
        <h2>{{ name }} color</h2>
        <p>{{ description }}</p>
        <UColorPicker :model-value="color" format="hex" size="sm" @update:model-value="apply" />
        <label>
          Hex color
          <input
            v-model="hex"
            type="text"
            maxlength="7"
            autocomplete="off"
            spellcheck="false"
            :aria-invalid="invalid || undefined"
            @keydown.enter.prevent="apply(hex)"
          />
        </label>
        <p v-if="invalid" role="status">Enter a hex color such as #4fa2dc.</p>
        <div class="atlas-resource-color-actions">
          <button type="button" @click="apply(hex)">Apply</button>
          <button type="button" :disabled="!custom" @click="reset">Reset color</button>
        </div>
      </div>
    </template>
  </UPopover>
</template>

<style scoped>
.atlas-resource-swatch {
  display: grid;
  flex: none;
  width: 1.5rem;
  height: 1.5rem;
  place-items: center;
  border: 1px solid #47616b;
  border-radius: 0.35rem;
  cursor: pointer;
}
.atlas-resource-swatch span {
  width: 0.7rem;
  height: 0.7rem;
  border-radius: 0.13rem;
}
.atlas-resource-swatch:hover,
.atlas-resource-swatch[aria-expanded="true"] {
  background: #274854;
}
.atlas-resource-swatch:focus-visible {
  outline: 2px solid #f0c78c;
}
:global(.atlas-resource-color-flyout) {
  max-width: calc(100vw - 1rem);
  border: 1px solid #47616b;
  border-radius: 0.55rem;
  background: #132b37;
  color: #e0edf0;
}
.atlas-resource-color-content {
  display: grid;
  gap: 0.5rem;
  padding: 0.75rem;
  font-size: 0.8rem;
  max-width: 16rem;
}
.atlas-resource-color-content h2 {
  font-weight: 600;
}
.atlas-resource-color-content p {
  color: #a9c0c9;
}
.atlas-resource-color-content label {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}
.atlas-resource-color-content input {
  width: 6rem;
  border: 1px solid #47616b;
  border-radius: 0.25rem;
  padding: 0.25rem 0.4rem;
  font-family: ui-monospace, monospace;
}
.atlas-resource-color-actions {
  display: flex;
  gap: 0.75rem;
  color: #f0c78c;
}
.atlas-resource-color-actions button {
  text-decoration: underline;
  cursor: pointer;
}
.atlas-resource-color-actions button:disabled {
  opacity: 0.45;
  cursor: default;
}
</style>
