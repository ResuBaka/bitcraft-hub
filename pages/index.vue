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
  <bitcraft-leaderboard></bitcraft-leaderboard>
</template>
