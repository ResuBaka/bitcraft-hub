<script setup lang="ts">
import skills from "~/logic/skills";
const tiers = ref<number[]>([1, 2, 3, 4, 5]);

const { data } = await useFetch<Item[]>("/api/buildings");

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

const deleteBuilding = (item: Item) => {
  const index = data.value?.findIndex((a) => a.id === item.id);

  if (!index) {
    return;
  }

  if (index > -1) {
    data.value?.splice(index, 1);
  }
};

const filteredItems = computed(() => {
  if (!search.value) {
    return data.value;
  }

  return data.value?.filter((a) =>
    a.title.toLowerCase().includes(search.value.toLowerCase()),
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
      <building @delete="deleteBuilding" :building="item"></building>
    </v-col>
  </v-row>
  </v-container>
</template>
