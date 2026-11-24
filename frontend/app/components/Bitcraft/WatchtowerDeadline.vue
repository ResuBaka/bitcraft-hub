<script setup lang="ts">
import type { Watchtower } from "~/types/Watchtower";
import { watchtowerEmptyBy } from "~/utils/watchtowerEnergy";

const props = defineProps<{ tower: Watchtower; fetchedAt: number | null }>();
const deadline = computed(() => watchtowerEmptyBy(props.tower, props.fetchedAt));
</script>

<template>
  <LocalDateTime v-if="deadline instanceof Date" :datetime="deadline" />
  <span v-else>{{ deadline }}</span>
</template>
