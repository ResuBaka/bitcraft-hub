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

const { data: playerFetch, pending: playerPnding } = useFetch(() => {
  console.log(`/api/bitcraft/players/${route.params.id}`);
  return `/api/bitcraft/players/${route.params.id}`;
});
const { data: inventoryFetch, pending: inventoryPending } = useFetch(() => {
  return `/api/bitcraft/inventorys?owner_entity_id=${route.params.id}`;
});

const { data: experienceFetch } = useFetch(() => {
  console.log(`/api/bitcraft/experience/${route.params.id}`);
  return `/api/bitcraft/experience/${route.params.id}`;
});

const expeirence = computed(() => {
  return experienceFetch.value ?? undefined;
});
console.log(expeirence);
const inventorys = computed(() => {
  return inventoryFetch.value?.inventorys ?? [];
});

const player = computed(() => {
  return playerFetch.value ?? undefined;
});
</script>

<template>
  <v-layout class="justify-center" v-if="playerPnding">
    <v-progress-circular  indeterminate >
    </v-progress-circular>
  </v-layout>
  <template v-else-if="player">
    <v-banner class="text-high-emphasis font-weight-black">Player: {{ player?.username }}</v-banner>
    <v-card>
      <v-card-text  :class="computedClass">
        <v-table :class="computedClass" density="compact">
          <tbody>
          <tr style='text-align: right'>
            <th>signed_in:</th>
            <td>{{player.signed_in}}</td>
          </tr>
          <tr style='text-align: right'>
            <th>sign_in_timestamp:</th>
            <td>{{ player.sign_in_timestamp }}</td>
          </tr>
          <tr style='text-align: right'>
            <th>session_start_timestamp:</th>
            <td>{{ player.session_start_timestamp }}</td>
          </tr>
          <tr style='text-align: right'>
            <th>time_played:</th>
            <td>{{ player.time_played }}</td>
          </tr>
          <tr style='text-align: right'>
            <th>time_signed_in:</th>
            <td>{{ player.time_signed_in }}</td>
          </tr>
          </tbody>
        </v-table>
        <v-table :class="computedClass" density="compact">
          <tbody v-if="expeirence !== undefined">
            <tr style='text-align: right'>
              <th>Skills</th>
              <td>Experience</td>
              <td>Level</td>
              <td>Rank</td>
            </tr>
            <tr v-for="[skill,xp_info] of Object.entries(expeirence.experience_stacks)" style='text-align: right'>
              <th>{{skill}}</th>
              <td>{{xp_info.experience}}</td>
              <td>{{xp_info.level}}</td>
              <td>#{{xp_info.rank}}</td>
            </tr>
          </tbody>
        </v-table>
      </v-card-text>
    </v-card>

    <v-card variant="text">
      <v-card-title>Inventory's</v-card-title>
      <v-card-text>
        <v-row>
        <template v-if="!inventoryPending" v-for="(inventory, index) in inventorys">
          <v-col cols="12" md="6">
            <bitcraft-inventory :inventory="inventory"></bitcraft-inventory>
          </v-col>
        </template>
        <v-layout class="justify-center" v-else>
          <v-progress-circular  indeterminate >
          </v-progress-circular>
        </v-layout>
        </v-row>
      </v-card-text>
    </v-card>
  </template>
  <template v-else>
    <v-alert type="error">Player not found</v-alert>
  </template>
</template>

<style scoped>
</style>