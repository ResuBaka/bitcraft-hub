<script setup lang="ts">
const props = defineProps<{
  items: any;
}>();
</script>

<template>
  <template v-for="options of items">
    <v-list-item v-for="item of options">
<v-list-item-title>Quantity {{ item.quantity }}</v-list-item-title>
<v-list-item-title > Name {{ item.item_id }} </v-list-item-title>
  <bitcraft-item-stack :items="item.inner" v-if="item.inner !== undefined"></bitcraft-item-stack>
</v-list-item>
</template>
</template>