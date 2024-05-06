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

const inventorys = computed(() => {
  return inventoryFetch.value?.inventorys ?? [];
});

const player = computed(() => {
  return playerFetch.value ?? undefined;
});

</script>

<template>
  <v-card  v-if="player !== undefined">
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
</template>

<style scoped>
</style>