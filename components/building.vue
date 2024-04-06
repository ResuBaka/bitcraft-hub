<script setup lang="ts">
defineProps<{
  building: Building;
}>();

const emit = defineEmits(["delete"]);

const deleteItem = (building: Building) => {
  emit("delete", building);
};
</script>

<template>
  <v-card>
    <v-card-title><nuxt-link :to="{ name: 'buildings-id', params: { id: building.id } }">{{ building.title }}</nuxt-link></v-card-title>
    <v-card-text>
      <p v-if="building.id" >Id: {{ building.id }}</p>
      <p v-if="building.tier" >Tier: {{ building.tier }}</p>
    </v-card-text>
    <v-btn @click="deleteItem(building)">Delete</v-btn>
  </v-card>
</template>

<style scoped>

</style>