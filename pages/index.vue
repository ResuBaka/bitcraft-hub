<script setup lang="ts">
const tiers = ref<number[]>([1, 2, 3, 4, 5]);
import Fuse from "fuse.js";

const { data, refresh: refreshItems } = await useFetch<Item[]>("/api/items");

const items = computed(() => {
  return data.value;
});

const skills = ref<Skill[]>([
  {
    title: "Woodcutting",
    id: "woodcutting",
  },
  {
    title: "Farming",
    id: "farming",
  },
  {
    title: "Herblore",
    id: "herblore",
  },
  {
    title: "Hunter",
    id: "hunter",
  },
  {
    title: "Mining",
    id: "mining",
  },
  {
    title: "Smithing",
    id: "smithing",
  },
  {
    title: "Fishing",
    id: "fishing",
  },
  {
    title: "Cooking",
    id: "cooking",
  },
  {
    title: "Fletching",
    id: "fletching",
  },
]);

const newItem = ref({
  title: "",
  id: "",
  type: "",
  items: [],
  building: "",
  skill: undefined,
  skillLevel: undefined,
  tier: 1,
});

const valid = ref(false);
const customId = ref(false);
const fuse = new Fuse(items.value, {
  keys: ["title"],
});
watch(
  () => items.value,
  () => {
    if (items.value) {
      console.log("items.value");
      fuse.setCollection(items.value);
    }
  },
);

const addItem = async () => {
  if (!newItem.value.title) {
    return;
  }

  if (!newItem.value.id) {
    newItem.value.id = newItem.value.title.toLowerCase().replace(" ", "_");
  }

  await fetch("/api/items", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(newItem.value),
  });

  const latestData = await useFetch<Item[]>("/api/items");

  if (latestData.data.value) {
    data.value = latestData.data.value;
  }

  newItem.value = {
    title: "",
    id: "",
    type: "",
    items: [],
    building: "",
    skill: undefined,
    skillLevel: undefined,
    tier: 1,
  };
};

const addNewNeedItem = () => {
  newItem.value.items.push({
    id: "",
    amount: 1,
  });
};

const spaceRegex = /\s/g;

watch(
  () => newItem.value.title,
  () => {
    if (!customId.value) {
      newItem.value.id = newItem.value.title
        .toLowerCase()
        .replace(spaceRegex, "_");
    }
  },
);

const search = ref("");

const addItemDialog = ref(false);

const toggleItemDialog = () => {
  addItemDialog.value = true;
};

const deleteItem = (item: Item) => {
  if (confirm("Are you sure you want to delete this item?")) {
    fetch(`/api/items/${item.id}`, {
      method: "DELETE",
    });
    refreshItems();
  }
};

const filteredItems = computed(() => {
  if (!search.value) {
    return items.value;
  }

  return fuse.search(search.value).map((result) => result.item);
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
