<script setup lang="ts">
const { data: items } = useFetch('/api/bitcraft/items')

const page = ref(1)
const perPage = 30

const currentItems = computed(() => {
  return searchItems.value?.slice((page.value - 1) * perPage, page.value * perPage) ?? []
})

const tag = ref(null)
const tier = ref(null)
const search = ref<string | null>('')

const searchItems = computed(() => {
  return items.value?.filter((item: any) => {
    return (!tag.value || item.tag === tag.value) &&
        (!tier.value || item.tier === tier.value) &&
        (!search.value || item.name.toLowerCase().includes(search.value.toLowerCase()))
  }) ?? []
})


const tags = ref([])

watch(() => items.value, () => {
  tags.value = items.value?.reduce((acc: string[], item: any) => {
    if (!acc.includes(item.tag)) {
      acc.push(item.tag)
    }
    return acc
  }, []) ?? []

  tags.value.sort()
})

const tiers = ref([])

watch(() => items.value, () => {
  tiers.value = items.value?.reduce((acc: string[], item: any) => {
    if (!acc.includes(item.tier)) {
      acc.push(item.tier)
    }
    return acc
  }, []) ?? []

  tiers.value.sort((a, b) => a > b ? 1 : -1)
})

const length = computed(() => {
  return Math.ceil(searchItems.value?.length / perPage) ?? 0
})

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
    <v-col>
      <v-autocomplete
          v-model="tag"
          :items="tags"
          label="Tag"
          outlined
          dense
          clearable
      ></v-autocomplete>
    </v-col>
    <v-col>
      <v-select
          v-model="tier"
          :items="tiers"
          label="Tier"
          outlined
          dense
          clearable
      ></v-select>
    </v-col>
  </v-row>
  <v-row>
    <pre class="">{{ items?.[0] }}
    </pre>
  </v-row>
  <v-row>
    <v-col>{{ tags.length }}</v-col>
  </v-row>
  <v-row>
    <v-col>
      <v-pagination
          v-model="page"
          :length="length"
      ></v-pagination>
    </v-col>
  </v-row>
  <v-row>
    <v-col cols="12" md="3" v-for="item in currentItems" :key="item.id">
      <bitcraft-card-item :item="item"></bitcraft-card-item>
    </v-col>
  </v-row>
</template>

<style scoped>
</style>