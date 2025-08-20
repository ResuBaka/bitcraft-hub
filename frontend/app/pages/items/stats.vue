<script setup lang="ts">
const page = ref(1);

const tag = ref<string | null>(null);
const tier = ref<number | null>(null);
const search = ref<string | null>("");
const debouncedSearch = ref<string | null>("");

const route = useRoute();

debouncedSearch.value = (route.query.search as string) ?? "";
search.value = debouncedSearch.value;
tag.value = (route.query.tag as string) ?? null;

if (route.query.tier) {
  tier.value = parseInt(route.query.tier);
}

const { data, pending } = await useLazyFetchMsPack<{
  items: any[];
  cargo: any[];
}>(() => {
  return `/inventory/all_inventory_stats`;
});

const { data: metaData } = await useLazyFetchMsPack(() => {
  return `/api/bitcraft/itemsAndCargo/meta`;
});

const items = computed(() => {
  if (!data || !data?.value) {
    return [];
  }
  if (!debouncedSearch.value) {
    return Object.values(data.value.items).splice(0, 19);
  }

  return Object.values(data.value.items).filter((value) => {
    if (
      value[1].name
        .toLocaleLowerCase()
        .includes(debouncedSearch.value?.toLocaleLowerCase())
    ) {
      return true;
    }

    if (
      value[1].rarity
        .toLocaleLowerCase()
        .includes(debouncedSearch.value?.toLocaleLowerCase())
    ) {
      return true;
    }

    return false;
  });
});

const cargo = computed(() => {
  if (!data || !data?.value) {
    return [];
  }
  if (!debouncedSearch.value) {
    return Object.values(data.value.cargo).splice(0, 19);
  }

  return Object.values(data.value.cargo).filter((value) => {
    if (
      value[1].name
        .toLocaleLowerCase()
        .includes(debouncedSearch.value?.toLocaleLowerCase())
    ) {
      return true;
    }

    if (
      value[1].rarity
        .toLocaleLowerCase()
        .includes(debouncedSearch.value?.toLocaleLowerCase())
    ) {
      return true;
    }

    return false;
  });
});

const numberFormat = new Intl.NumberFormat();

useSeoMeta({
  title: () => `Inventory Stats`,
});
</script>

<template>
  <v-container fluid>
    <v-row>
      <v-col>
        <v-text-field
            v-model="debouncedSearch"
            label="Search"
            outlined
            dense
            clearable
        ></v-text-field>
      </v-col>
<!--      <v-col>-->
<!--        <v-autocomplete-->
<!--            v-model="tag"-->
<!--            :items="metaData?.tags || []"-->
<!--            label="Tag"-->
<!--            outlined-->
<!--            dense-->
<!--            clearable-->
<!--        ></v-autocomplete>-->
<!--      </v-col>-->
<!--      <v-col>-->
<!--        <v-select-->
<!--            v-model="tier"-->
<!--            :items="metaData?.tiers || []"-->
<!--            label="Tier"-->
<!--            outlined-->
<!--            dense-->
<!--            clearable-->
<!--        ></v-select>-->
<!--      </v-col>-->
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
    <template v-if="data">
      <v-row>
        <v-col cols="12" md="6" lg="4" xl="3" v-for="item in items" :key="`item:${item[1].id}`">
          {{ numberFormat.format(item[0]) }} <b>{{ item[1].name }}</b> {{ item[1].rarity }} {{ item[1].tier > 0 ? item[1].tier : '' }}
        </v-col>
      </v-row>
      <v-divider class="mt-5 pb-5"  thickness="10" />
      <v-row>
        <v-col cols="12" md="6" lg="4" xl="3" v-for="item in cargo" :key="`cargo:${item[1].id}`">
          {{ numberFormat.format(item[0]) }} <b>{{ item[1].name }}</b> {{ item[1].rarity }} {{ item[1].tier > 0 ? item[1].tier : '' }}
        </v-col>
      </v-row>
      <v-row>
        <v-col cols="12">
          <v-pagination
              @update:model-value="changePage"
              v-model="page"
              :length="data?.pages || 0"
          ></v-pagination>
        </v-col>
      </v-row>
    </template>
    <template v-else>
      <v-empty-state
       headline="No items found"
       text="Try changing your search criteria"
      ></v-empty-state>
    </template>
  </v-container>
</template>
