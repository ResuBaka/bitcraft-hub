<script setup lang="ts">
const route = useRoute();

const { data } = await useFetch<FullItem>(`/api/items/${route.params.id}`);

const resourcesFlat = computed<FullItem[]>(() => {
  const items: FullItem[] = [];

  const addItems = (localItem: FullItem) => {
    localItem.items.forEach((resource) => {
      items.push(resource);
      if (resource.items) {
        addItems(resource);
      }
    });
  };

  if (data.value) {
    addItems(data.value);
  }

  return items;
});

const numberTosToCrate = ref(1);

const itemNeededForCrafting = computed(() => {
  const items: FullItem[] = [];

  resourcesFlat.value.forEach((resource) => {
    if (resource.items) {
      items.push(resource);
    }
  });

  return items;
});
</script>

<template>
<v-row>
<v-container fluid>
  <v-card>
    <v-card-title>Name: {{ data.title }}</v-card-title>
    <v-card-text>
      <p>Building: {{ data.building }}</p>
      <p v-if="data.skill" >Skill: {{ data.skill }}</p>
      <p v-if="data.creates" >Creates: {{ data.creates }}</p>
      <p v-if="data.tier" >Tier: {{ data.tier }}</p>
      <p v-if="data.tool">Tools: {{ data.tool }}</p>
    </v-card-text>
    <v-card-actions>
      <v-btn :to="{name: 'items-edit-id', params: {id: data.id}}">Go to Update Page</v-btn>
    </v-card-actions>
<!--    <v-btn @click="deleteItem(item)">Delete</v-btn>-->
  </v-card>
</v-container>
</v-row>

  <v-row>
    <v-container fluid>
      <v-card>
        <v-card-title>Items</v-card-title>
        <v-card-text>
          <v-row>
            <v-col v-for="resource in itemNeededForCrafting" :key="resource.id" cols="12" md="4">
              <p>Title {{resource.title}}}</p>
              <p>Title {{resource.amount}}</p>
            </v-col>
          </v-row>
        </v-card-text>
      </v-card>
    </v-container>
  </v-row>
  <v-row>
    <v-container fluid>
      <v-card>
        <v-card-title>All needed Items</v-card-title>
        <v-card-text>
          <v-row>
            <v-col v-for="resource in resourcesFlat" :key="resource.id" cols="12" md="4">
              <item :item="resource"></item>
            </v-col>
          </v-row>
        </v-card-text>
      </v-card>
    </v-container>
  </v-row>
</template>

<style scoped>

</style>