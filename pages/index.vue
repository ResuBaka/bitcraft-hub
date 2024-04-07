<script setup lang="ts">
import type { Item } from "~/types";

const items = ref<Item[]>([]);
const customId = ref(false);
const search = ref("");

const deleteItem = (item: Item) => {
  if (confirm("Are you sure you want to delete this item?")) {
    // fetch(`/api/items/${item.id}`, {
    //   method: "DELETE",
    // });
    // refreshItems();
  }
};

const filteredItems = computed(() => {
  if (!search.value) {
    return items.value;
  }

  return (
    items.value?.filter((item) => {
      return item.name.toLowerCase().includes(search.value.toLowerCase());
    }) || []
  );
});
</script>

<template>
  <v-container>
  <v-row>
    <v-col cols="12">
      <v-text-field v-model="search"></v-text-field>
    </v-col>
  </v-row>

  <v-row>
    <v-col v-for="item in filteredItems" :key="item.id" cols="6" md="4">
      <item @delete="deleteItem" :item="item"></item>
    </v-col>
  </v-row>
  </v-container>
</template>
