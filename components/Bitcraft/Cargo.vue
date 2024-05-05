<script setup lang="ts">
const props = defineProps<{
    cargo: any;
  }>();

</script>

<template>

<v-card-title > Name </v-card-title>
<v-card-text  >{{ cargo.name }}</v-card-text >
</template>