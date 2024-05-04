<script setup lang="ts">
import { watchThrottled } from "@vueuse/shared";

const page = ref(1);
const perPage = 30;

const search = ref<string | null>("");

const route = useRoute();
const router = useRouter();

const tmpPage = (route.query.page as string) ?? null;

if (tmpPage) {
  page.value = parseInt(tmpPage);
}

const { data: inventoryFetch, pending: InventoryPending } = useFetch(() => {
  console.log(`/api/bitcraft/inventorys/${route.params.id}`);
  return `/api/bitcraft/inventorys/${route.params.id}`;
});

const { data: InventoryChangesFetch, pending: InventoryChangesPending } =
  useFetch(() => {
    console.log(`/api/bitcraft/inventorys/changes${route.params.id}`);
    return `/api/bitcraft/inventorys/changes/${route.params.id}`;
  });

const inventory = computed(() => {
  return inventoryFetch.value ?? undefined;
});
const inventoryChanges = computed(() => {
  return InventoryChangesFetch.value ?? [];
});
</script>

<template>
    <v-card  v-if="inventory !== undefined">
    <v-toolbar color="transparent">
      <v-toolbar-title >{{ inventory.entity_id }}</v-toolbar-title>

    </v-toolbar>

    <v-card-text>
      <v-card-title>Current State</v-card-title>
      <v-row>
        
    <v-col cols="12" md="2" v-for="items in inventory.pockets">
      <v-card v-if="items.contents !== undefined">
                
        <bitcraft-item :item="items.contents"></bitcraft-item>
                </v-card>
    </v-col>
  </v-row>
  <v-card-title>Changes</v-card-title>

  <v-row >
        <v-col cols="12" md="2" v-for="inventoryChange in inventoryChanges">
          <template v-for="items of inventoryChange.diff">
            <v-card>
            <v-card-title >Player</v-card-title>
                    <v-card-subtitle >{{ inventoryChange.playerName }}</v-card-subtitle>
              </v-card>
                    <v-card  v-if="items.new !== undefined">
                    <v-card-title >new</v-card-title>
                    <bitcraft-item :item="items.new"></bitcraft-item>
                  </v-card>
                  <v-card v-if="items.old !== undefined">
                    <v-card-title >Old</v-card-title>
                    <bitcraft-item :item="items.old"></bitcraft-item>
                  </v-card>
                </template>
        </v-col>
      </v-row>
    </v-card-text>
    
  </v-card>
</template>

<style scoped>
</style>