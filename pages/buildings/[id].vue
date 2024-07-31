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
const {
  public: { api },
} = useRuntimeConfig();
const { new_api } = useConfigStore();

const { data: buildingsFetch, pending: buildingPending } = useFetch(() => {
  if (new_api) {
    return `${api.base}/buildings/${route.params.id}`;
  } else {
    return `/api/bitcraft/buildings/${route.params.id}`;
  }
});

const { data: inventoryFetch, pending: inventoryPending } = useFetch(() => {
  if (new_api) {
    return `${api.base}/api/bitcraft/inventorys/owner_entity_id/${route.params.id}`;
  } else {
    return `/api/bitcraft/inventorys?owner_entity_id=${route.params.id}`;
  }
});

const building = computed(() => {
  return buildingsFetch.value ?? undefined;
});
const inventorys = computed(() => {
  return inventoryFetch.value?.inventorys ?? [];
});
</script>

<template>
  <v-container fluid>
    <v-card v-if="building !== undefined">
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
                <bitcraft-inventory :inventory="inventory"></bitcraft-inventory>
              </v-list-item>
            </v-list-item>
          </v-list>
        </v-list>
      </v-card-text>
    </v-card>
  </v-container>
</template>
