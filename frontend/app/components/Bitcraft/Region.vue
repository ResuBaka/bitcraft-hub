<script setup lang="ts">
import { regions } from "~/consts/regions";
const configStore = useConfigStore();

const props = defineProps<{
  region: number;
}>();
</script>

<template>
  <template v-if="configStore.show_region_as_number">
    {{ region }}
  </template>
  <template v-else>
    {{ regions[region] }}
  </template>
</template>

<style scoped></style>
