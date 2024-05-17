<script setup lang="ts">
const props = defineProps<{
  item: any;
}>();
</script>

<template>
  <v-card-title>Quantity</v-card-title>
  <v-card-text>{{ item.quantity }}</v-card-text>

  <v-card-title> Name</v-card-title>
  <v-card-text>{{ item.item.name }}</v-card-text>
</template>