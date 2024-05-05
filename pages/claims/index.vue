<script setup lang="ts">
import { watchThrottled } from "@vueuse/shared";

const page = ref(1);
const perPage = 16;

const search = ref<string | null>("");

const route = useRoute();
const router = useRouter();

const tmpPage = (route.query.page as string) ?? null;

if (tmpPage) {
  page.value = parseInt(tmpPage);
}

const { data: claims, pending } = useFetch(() => {
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
    return `/api/bitcraft/claims?${querys}`;
  }

  return `/api/bitcraft/claims`;
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

const currentClaims = computed(() => {
  return claims.value?.claims ?? [];
});

const length = computed(() => {
  return Math.ceil(claims.value?.total / perPage) ?? 0;
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
      <v-pagination
          v-model="page"
          :length="length"
      ></v-pagination>
      <v-progress-linear
          color="yellow-darken-2"
          indeterminate
          :active="pending"
      ></v-progress-linear>
    </v-col>
  </v-row>
  <v-row>
    <v-col cols="12" md="3" v-for="claim in currentClaims" :key="claim.entity_id">
      <bitcraft-card-claim :claim="claim"></bitcraft-card-claim>
    </v-col>
  </v-row>
</template>

<style scoped>
</style>