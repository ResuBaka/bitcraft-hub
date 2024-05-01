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

const { data: claimFetch, claimPnding } = useFetch(() => {
  console.log(`/api/bitcraft/claims/${route.params.id}`);
  return `/api/bitcraft/claims/${route.params.id}`;
});
const { data: buidlingsFetch, buildingsPending } = useFetch(() => {
  console.log(`/api/bitcraft/buildings/?${route.params.id}`);
  return `/api/bitcraft/buildings?claim_entity_id=${route.params.id}&with_inventory=true`;
});

const claim = computed(() => {
  return claimFetch.value ?? undefined;
});
const buildings = computed(() => {
  return buidlingsFetch.value?.buildings ?? [];
});

console.log(claim);
</script>

<template>
    <v-card  v-if="claim !== undefined">
    <v-toolbar color="transparent">
      <v-toolbar-title>
        >{{ claim.name }} : {{ claim.entity_id }}</v-toolbar-title>

    </v-toolbar>

    <v-card-text>
        <v-list>
          <v-list>
          <v-list-item>
            <v-list-item-title>Owner</v-list-item-title>
            <v-list-item-subtitle>{{ claim.owner_player_entity_id }}</v-list-item-subtitle>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>Supplies</v-list-item-title>
            <v-list-item-subtitle>{{ claim.supplies }}</v-list-item-subtitle>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>Tiles</v-list-item-title>
            <v-list-item-subtitle>{{ claim.tiles }}</v-list-item-subtitle>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>Location</v-list-item-title>
            <v-list-item-subtitle>{{ claim.location }}</v-list-item-subtitle>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>Members</v-list-item-title>
            <v-list-item v-for="member in claim.members" :key="member.user_name">
              <v-list-item-subtitle>{{ member.user_name }}</v-list-item-subtitle>
          </v-list-item>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>Buildings</v-list-item-title>
            <v-list-item v-for="building in buildings" :key="building.entity_id">
              <v-list-item-subtitle v-if="building.nickname !== ''">{{ building.nickname }}</v-list-item-subtitle>
              <v-list-item-subtitle v-else>{{ building.entity_id }}</v-list-item-subtitle>
          </v-list-item>
          </v-list-item>
        </v-list>
        </v-list>
    </v-card-text>
  </v-card>
</template>

<style scoped>
</style>