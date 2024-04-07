<script setup lang="ts">
import type { Item } from "~/types";

defineProps<{
  item: Item;
}>();

const emit = defineEmits(["delete"]);

const deleteItem = (item: Item) => {
  emit("delete", item);
};
</script>

<template>
  <v-card>
    <v-card-title><nuxt-link :to="{ name: 'items-id', params: { id: item.id } }">{{ item.name }}</nuxt-link></v-card-title>
    <v-card-text>
    </v-card-text>
    <v-btn @click="deleteItem(item)">Delete</v-btn>
  </v-card>
</template>

<style scoped>

</style>