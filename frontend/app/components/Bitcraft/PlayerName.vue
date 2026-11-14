<script setup lang="ts">
const props = defineProps<{ playerId: string | number | bigint | null | undefined }>();
const config = useConfigStore();
const highlighted = computed(() => config.isPlayerHighlighted(props.playerId));
</script>

<template>
  <span class="inline-flex items-baseline gap-1">
    <span><slot /></span>
    <UIcon v-if="highlighted" name="i-lucide-star" class="size-[0.85em] shrink-0 text-amber-500" aria-label="Highlighted player" />
  </span>
</template>
