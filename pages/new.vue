<script setup lang="ts">
import skills from "~/logic/skills";

const tiers = ref<number[]>([1, 2, 3, 4, 5]);

const { data, refresh: refreshItems } = await useFetch<Item[]>("/api/items");
const { data: buildings, refresh: refreshBuildings } =
  await useFetch<Item[]>("/api/buildings");

const items = computed(() => {
  return data.value;
});

const newItem = ref({
  title: "",
  id: "",
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

  const { status, data, pending } = await useFetch("/api/items", {
    method: "PUT",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(newItem.value),
  });

  if (status.value === "success") {
    useRouter().push({ name: "items-id", params: { id: newItem.value.id } });
  }
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

const addItemDialog = ref(false);

const toggleItemDialog = () => {
  addItemDialog.value = true;
};

const deleteItem = (item: Item) => {
  const index = items.value.findIndex((a) => a.id === item.id);

  if (index > -1) {
    items.value.splice(index, 1);
  }
};

const newBuilding = ref({
  title: "",
  id: "",
  tier: 1,
  items_can_be_crafted: [],
});

const addBuilding = async () => {
  if (!newBuilding.value.title) {
    return;
  }

  if (!newBuilding.value.id) {
    newBuilding.value.id = newBuilding.value.title
      .toLowerCase()
      .replace(" ", "_");
  }

  await fetch("/api/buildings", {
    method: "PUT",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(newBuilding.value),
  });

  await refreshBuildings();

  newBuilding.value = {
    title: "",
    id: "",
    tier: 1,
    items_can_be_crafted: [],
  };
};

watch(
  () => newBuilding.value.title,
  () => {
    newBuilding.value.id = newBuilding.value.title
      .toLowerCase()
      .replace(spaceRegex, "_");
  },
);
</script>

<template>
  <v-container>
    <v-card>
      <v-card-title>Add Item</v-card-title>
      <v-card-text>
        <v-container>
          <v-form v-model="valid">
            <v-row>
              <v-col cols="12" md="4">
                <v-text-field v-model="newItem.title" label="Title"/>
              </v-col>
              <v-col cols="12" md="4">
                <v-text-field readonly v-model="newItem.id" label="Item ID"/>
              </v-col>
              <v-col cols="12" md="4">
                <v-autocomplete v-model="newItem.building" :items="buildings.map(a => ({ title: a.title, value: a.id }))" label="Building"/>
              </v-col>
              <v-col cols="12" md="4">
                <v-autocomplete v-model="newItem.skill" :items="skills.map(a => ({ title: a.title, value: a.id }))"
                                label="Skill"/>
              </v-col>
              <v-col v-if="newItem.skill" cols="12" md="4">
                <v-text-field v-model="newItem.skillLevel" label="Skill Level"/>
              </v-col>
              <v-col cols="12" md="4">
                <v-select v-model="newItem.tier" :items="tiers" label="Tier"/>
              </v-col>
            </v-row>
            <v-row>
              <v-col cols="12" md="4">
                <v-btn @click="addNewNeedItem">Add Need Item</v-btn>
              </v-col>
            </v-row>
            <v-row>
              <v-col v-for="need in newItem.items" :key="need.id" cols="12" md="4">
                <v-select v-model="need.id" :items="items.map(a => ({ title: a.title, value: a.id }))" label="Item"/>
                <v-text-field v-model="need.amount" label="Amount"/>
              </v-col>
            </v-row>
            <v-row>
              <v-btn @click="addItem">Add</v-btn>
            </v-row>
          </v-form>
        </v-container>
      </v-card-text>
    </v-card>
  </v-container>
  <v-container>
    <v-card>
      <v-card-title>Add Building</v-card-title>
      <v-card-text>
        <v-container>
          <v-form v-model="valid">
            <v-row>
              <v-col cols="12" md="4">
                <v-text-field v-model="newBuilding.title" label="Title"/>
              </v-col>
              <v-col cols="12" md="4">
                <v-text-field readonly v-model="newBuilding.id" label="Build ID"/>
              </v-col>
              <v-col cols="12" md="4">
                <v-autocomplete v-model="newBuilding.items_can_be_crafted"
                                :items="items.map(a => ({ title: a.title, value: a.id }))"
                                label="Items it can craft" multiple/>
              </v-col>
              <v-col v-if="newItem.skill" cols="12" md="4">
                <v-text-field v-model="newItem.skillLevel" label="Skill Level"/>
              </v-col>
              <v-col cols="12" md="4">
                <v-select v-model="newItem.tier" :items="tiers" label="Tier"/>
              </v-col>
            </v-row>
            <v-row>
              <v-btn @click="addBuilding">Add</v-btn>
            </v-row>
          </v-form>
        </v-container>
      </v-card-text>
    </v-card>
  </v-container>
</template>
