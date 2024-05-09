<script setup lang="ts">
import { watchThrottled } from "@vueuse/shared";

const page = ref(1);
const perPage = 24;

const search = ref<string | null>("");

const route = useRoute();
const router = useRouter();

const tmpPage = (route.query.page as string) ?? null;

if (tmpPage) {
  page.value = parseInt(tmpPage);
}

const { data: players, pending } = useFetch(() => {
  const url = new URLSearchParams();

  if (search.value) {
    url.append("search", search.value);
  }

  if (page.value) {
    url.append("page", page.value.toString());
  }

  if (perPage) {
    url.append("page", perPage.toString());
  }

  const querys = url.toString();

  if (querys) {
    return `/api/bitcraft/players?${querys}`;
  }

  return `/api/bitcraft/players`;
});

watchThrottled(
  () => [search.value, page.value],
  () => {
    const newQuery = {};

    if (search.value) {
      newQuery.search = search.value;
    }

    if (page.value) {
      newQuery.page = page.value;
    }

    router.push({ query: newQuery });
  },
  { throttle: 50 },
);

const currentplayers = computed(() => {
  return players.value?.players ?? [];
});

const length = computed(() => {
  if (players.value?.total) {
    return Math.ceil(players.value?.total / perPage);
  }

  return 0;
});

const theme = useTheme();

const computedClass = computed(() => {
  return {
    "bg-surface-light": theme.global.current.value.dark,
    "bg-grey-lighten-3": !theme.global.current.value.dark,
  };
});
</script>

<template>
  <v-row>
    <v-col>
      <v-text-field
          v-model="search"
          label="Search"
          outlined
          dense
          clearable
      ></v-text-field>
    </v-col>
  </v-row>
  <v-row>
    <v-col>
      <v-progress-linear
          color="yellow-darken-2"
          indeterminate
          :active="pending"
      ></v-progress-linear>
    </v-col>
  </v-row>
  <v-row>
    <v-col cols="12" md="6" lg="4" xl="3" xxl="2" v-for="player in currentplayers" :key="player.entity_id">
      <v-card>
      <template v-slot:title>
        <nuxt-link class="text-decoration-none text-high-emphasis font-weight-black" :to="{ name: 'players-id', params: { id: player.entity_id } }"
        >{{ player.username }} : {{ player.entity_id }}</nuxt-link>
      </template>
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
    </v-card-text>
  </v-card>
    </v-col>
    <v-col cols="12">
      <v-pagination
          v-model="page"
          :length="length"
      ></v-pagination>
    </v-col>
  </v-row>
</template>

<style scoped>
</style>