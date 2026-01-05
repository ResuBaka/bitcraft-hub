<script setup lang="ts">
import { regions } from "~/consts/regions";
const configStore = useConfigStore();

const props = defineProps<{
  region: string;
}>();
</script>

<template>
  <template v-if="configStore.show_region_as_number">
    {{ region.replace("bitcraft-", "") }}
  </template>
  <template v-else>
    {{ regions[region.replace("bitcraft-", "")] }}
  </template>
</template>

<style scoped>

</style>