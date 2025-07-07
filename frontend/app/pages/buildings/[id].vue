<script setup lang="ts">
const route = useRoute();

const { data: buildingsFetch } = useFetchMsPack(() => {
  return `/buildings/${route.params.id}`;
});

const { data: inventoryFetch } = useFetchMsPack(() => {
  return `/api/bitcraft/inventorys/owner_entity_id/${route.params.id}`;
});

const inventorys = computed(() => {
  return inventoryFetch.value?.inventorys ?? [];
});

useSeoMeta({
  title: "Building Info",
});
</script>

<template>
  <v-container fluid>
    <v-card v-if="buildingsFetch">
      <v-toolbar color="transparent">
        <v-toolbar-title v-if="buildingsFetch.nickname !== ''">{{ buildingsFetch.nickname }}</v-toolbar-title>
        <v-toolbar-title v-else>{{ buildingsFetch.entity_id }}</v-toolbar-title>
      </v-toolbar>
      <v-card-text>
        <v-list>
          <v-list>
            <v-list-item>
              <v-list-item-title>Inventorys</v-list-item-title>
              <v-list-item v-for="inventory in inventorys">
                <bitcraft-inventory :inventory="inventory"></bitcraft-inventory>
              </v-list-item>
            </v-list-item>
          </v-list>
        </v-list>
      </v-card-text>
    </v-card>
  </v-container>
</template>
