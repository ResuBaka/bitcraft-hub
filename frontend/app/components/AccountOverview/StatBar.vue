<script setup lang="ts">
import { meterValue } from "~/utils/monitorBars";

const props = defineProps<{
  label: string;
  value: number | null;
  max: number | null | undefined;
  color: "health" | "stamina";
}>();
const value = computed(() => meterValue(props.value, props.max));
</script>

<template>
  <div class="min-w-0">
    <div class="flex flex-wrap justify-between gap-x-2 text-sm">
      <span class="font-medium">{{ label }}</span>
      <span class="text-muted">
        {{ props.value == null ? "Unavailable" : props.value.toLocaleString() }}
        <template v-if="value !== null && props.max != null">
          / {{ props.max.toLocaleString() }}</template
        >
        <template v-else-if="props.value != null"> · max unavailable</template>
      </span>
    </div>
    <progress
      v-if="value !== null"
      :value="value"
      :max="props.max!"
      :aria-label="label"
      :style="{ '--stat-bar-color': color === 'health' ? '#ef4444' : '#b69c00' }"
      class="stat-bar mt-0.5 block h-1.5 w-full"
    />
    <div v-else class="mt-0.5 h-1.5 rounded bg-muted" aria-hidden="true" />
  </div>
</template>

<style scoped>
.stat-bar {
  appearance: none;
  border: 0;
  border-radius: 9999px;
  overflow: hidden;
  background-color: var(--ui-bg-elevated);
  color: var(--stat-bar-color);
}

.stat-bar::-webkit-progress-bar {
  background-color: var(--ui-bg-elevated);
}

.stat-bar::-webkit-progress-value {
  background-color: var(--stat-bar-color);
}

.stat-bar::-moz-progress-bar {
  background-color: var(--stat-bar-color);
}
</style>
