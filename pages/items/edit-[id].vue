<script setup lang="ts">
import router from "#app/plugins/router";

const route = useRoute();
import skills from "~/logic/skills";

const originalItem = ref<FullItem>({
  title: "",
  id: "",
  items: [],
  building: undefined,
  skill: undefined,
  tier: 1,
});

const { data, refresh } = await useFetch<FullItem>(
  `/api/items/${route.params.id}`,
  {
    onResponse({ response }): Promise<void> | void {
      originalItem.value = response._data;
    },
  },
);

const { data: buildings, refresh: refreshBuildings } =
  await useFetch<Item[]>("/api/buildings");
const { data: items, refresh: refreshItems } =
  await useFetch<Item[]>("/api/items");

const itemItems = computed(() => {
  return (
    data.value?.items.map((item) => ({
      title: item.title,
      id: item.id,
      amount: item.amount,
    })) || []
  );
});

const valid = ref(false);

const updateItem = async () => {
  if (!confirm("Are you sure you want to update this item?")) {
    return;
  }

  if (!data.value.title) {
    return;
  }

  await fetch(`/api/items/${data.value.id}`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(data.value),
  });

  await refresh();
};

const addNewNeedItem = () => {
  data.value.items.push({
    id: "",
    amount: 1,
  });
};

const spaceRegex = /\s/g;

const addItemDialog = ref(false);

const toggleItemDialog = () => {
  addItemDialog.value = true;
};

const deleteItem = async () => {
  if (!confirm("Are you sure you want to delete this item?")) {
    return;
  }

  const { status } = await useFetch(`/api/items/${data.value.id}`, {
    method: "DELETE",
  });

  if (status.value !== "success") {
    return;
  }

  useRouter().push("/");
};

const deleteNeed = (need: string) => {
  data.value.items = data.value.items.filter((a) => a.id !== need);
};
</script>

<template>
  <v-container >
    <v-card>
      <v-card-title>Add Item</v-card-title>
      <v-card-text>
        <v-container>
          <v-form v-model="valid">
            <v-row>
              <v-col cols="12" md="4">
                <v-text-field v-model="data.title" label="Title"/>
              </v-col>
              <v-col cols="12" md="4">
                <v-text-field readonly v-model="data.id" label="Item ID"/>
              </v-col>
              <v-col cols="12" md="4">
                <v-autocomplete v-model="data.building" :items="buildings.map(a => ({ title: a.title, value: a.id }))" label="Building"/>
              </v-col>
              <v-col cols="12" md="4">
                <v-autocomplete v-model="data.skill" :items="skills.map(a => ({ title: a.title, value: a.id }))"
                                label="Skill"/>
              </v-col>
              <v-col v-if="data.skill" cols="12" md="4">
                <v-text-field v-model="data.skillLevel" label="Skill Level"/>
              </v-col>
              <v-col cols="12" md="4">
                <v-select v-model="data.tier" :items="tiers" label="Tier"/>
              </v-col>
            </v-row>
            <v-row>
              <v-col cols="12" md="4">
                <v-btn @click="addNewNeedItem">Add Need Item</v-btn>
              </v-col>
            </v-row>
            <v-row>
              <v-col v-for="need in itemItems" :key="need.id" cols="12" md="4">
                <v-select v-model="need.id" :items="items.map(a => ({ title: a.title, value: a.id }))" label="Item"/>
                <v-text-field v-model="need.amount" label="Amount"/>
                <v-btn @click="deleteNeed(need.id)">Delete</v-btn>
              </v-col>
            </v-row>
            <v-row>
              <v-btn @click="updateItem">Update</v-btn>
              <v-btn @click="deleteItem">Delete</v-btn>
            </v-row>
          </v-form>
        </v-container>
      </v-card-text>
    </v-card>
  </v-container>
</template>
