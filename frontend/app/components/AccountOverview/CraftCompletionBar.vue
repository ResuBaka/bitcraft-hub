<script setup lang="ts">
defineProps<{ description: string; value: number; max: number }>();
defineEmits<{ acknowledge: [] }>();
</script>

<template>
  <button
    type="button"
    class="relative flex h-4 w-full cursor-pointer items-center justify-end overflow-hidden rounded-full bg-elevated px-2 text-[10px] font-semibold leading-none text-highlighted hover:ring-1 hover:ring-success focus-visible:outline-2 focus-visible:outline-success"
    :title="description"
    :aria-label="description"
    @click="$emit('acknowledge')"
  >
    <span
      aria-hidden="true"
      class="completion-fill pointer-events-none absolute inset-y-0 left-0 overflow-hidden rounded-full bg-success/40"
      :style="{ width: `${(value / max) * 100}%` }"
    />
    <span class="relative">Mark seen</span>
  </button>
</template>

<style scoped>
@media (prefers-reduced-motion: no-preference) {
  .completion-fill::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(110deg, transparent 25%, rgb(255 255 255 / 35%), transparent 75%);
    animation: shimmer 2.8s ease-in-out infinite;
  }
  @keyframes shimmer {
    from {
      transform: translateX(-100%);
    }
    65%, to {
      transform: translateX(100%);
    }
  }
}
</style>
