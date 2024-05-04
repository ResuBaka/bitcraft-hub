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

const { data: buildingsFetch, pending: buildingPending } = useFetch(() => {
  console.log(`/api/bitcraft/buildings/${route.params.id}`);
  return `/api/bitcraft/buildings/${route.params.id}`;
});

const { data: inventoryFetch, pending: inventoryPending } = useFetch(() => {
  return `/api/bitcraft/inventorys?owner_entity_id=${route.params.id}`;
});

const building = computed(() => {
  return buildingsFetch.value ?? undefined;
});
const inventorys = computed(() => {
  return inventoryFetch.value?.buildings ?? [];
});
</script>

<template>
    <v-card  v-if="building !== undefined">
    <v-toolbar color="transparent">
      <v-toolbar-title v-if="building.nickname !== ''">{{ building.nickname }}</v-toolbar-title>
      <v-toolbar-title v-else>{{ building.entity_id }}</v-toolbar-title>

    </v-toolbar>

    <v-card-text>
        <v-list>
          <v-list>
          <v-list-item>
            <v-list-item-title>Inventorys</v-list-item-title>
            <v-list-item v-for="inventory in inventorys">
              <v-list-item-subtitle ><nuxt-link :href="'/inventorys/' + inventory.entity_id"
        >{{ inventory.entity_id }}</nuxt-link></v-list-item-subtitle>
          </v-list-item>
          </v-list-item>
        </v-list>
        </v-list>
    </v-card-text>
  </v-card>
</template>

<style scoped>
</style>