<script setup lang="ts">
const tiers = ref<number[]>([1, 2, 3, 4, 5]);

const { data } = await useFetch<Item[]>("/api/items");

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
  items: [],
  building: "",
  skill: undefined,
  skillLevel: undefined,
  tier: 1,
});

const valid = ref(false);
const customId = ref(false);

const addItem = async () => {
  console.log(newItem.value);
  if (!newItem.value.title) {
    return;
  }

  if (!newItem.value.id) {
    newItem.value.id = newItem.value.title.toLowerCase().replace(" ", "_");
  }

  console.log(JSON.stringify(newItem.value, null, 2));

  return;

  await fetch("/api/items", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(newItem.value),
  });

  const latestData = await useFetch<Item[]>("/api/items");

  if (latestData.data.value) {
    console.log(JSON.stringify(latestData.data.value, null, 2));
    data.value = latestData.data.value;
  }

  newItem.value = {
    title: "",
    id: "",
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
                <v-row>
                  <v-col cols="1">
                    <v-checkbox v-model="customId">Custom Id</v-checkbox>
                  </v-col>
                  <v-col cols="10">
                    <v-text-field :readonly="customId" v-model="newItem.id" label="ID"/>
                  </v-col>
                </v-row>
              </v-col>
              <v-col cols="12" md="4">
                <v-text-field v-model="newItem.building" label="Building"/>
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
</template>
