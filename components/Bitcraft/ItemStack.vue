<script setup lang="ts">
const props = defineProps<{
  items: any;
}>();

const contetentToShow = ref(0);
</script>

<template>
  <template v-for="(option, index) in items">
    <v-list-item-title>{{ option.name.replace("{0}", option.crafted_item_stacks[0].item.name).replace("{1}", option.crafted_item_stacks[0].item.name) }}</v-list-item-title>
    <v-list-item-title>consumed_item_stacks</v-list-item-title>
    <v-list-item v-for="item of option.consumed_item_stacks">
<v-list-item-title>Quantity {{ item.quantity }}</v-list-item-title>
<v-list-item-title v-if="item.item.name !== undefined" > Name {{ item.item.name }} </v-list-item-title>
<v-list-item-title v-else > Name {{ item.item_id }} </v-list-item-title>
  <bitcraft-item-stack :items="item.inner" v-if="item.inner !== undefined"></bitcraft-item-stack>
</v-list-item>
<v-list-item-title>crafted_item_stacks</v-list-item-title>
<v-list-item v-for="item of option.crafted_item_stacks">
<v-list-item-title>Quantity {{ item.quantity }}</v-list-item-title>
<v-list-item-title v-if="item.item.name !== undefined" > Name {{ item.item.name }} </v-list-item-title>
<v-list-item-title v-else > Name {{ item.item_id }} </v-list-item-title>
  <bitcraft-item-stack :items="item.inner" v-if="item.inner !== undefined"></bitcraft-item-stack>
</v-list-item>
</template>
</template>