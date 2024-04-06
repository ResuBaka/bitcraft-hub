<script setup lang="ts">
defineProps<{
  item: Item;
}>();

const emit = defineEmits(["delete"]);

const deleteItem = (item) => {
  emit("delete", item);
};
</script>

<template>
  <v-card>
    <v-card-title><nuxt-link :to="{ name: 'items-id', params: { id: item.id } }">{{ item.title }}</nuxt-link></v-card-title>
    <v-card-text>
      <p>Building: {{ item.building }}</p>
      <p v-if="item.skill" >Skill: {{ item.skill }}</p>
      <p v-if="item.tier" >Tier: {{ item.tier }}</p>
      <p v-if="item.tool">Tools: {{ item.tool }}</p>
      <p v-if="item.creates">Creates: {{ item.creates }}</p>
    </v-card-text>
    <v-btn @click="deleteItem(item)">Delete</v-btn>
    <v-btn :to="{ name: 'items-edit-id', params: { id: item.id } }">Update</v-btn>
  </v-card>
</template>

<style scoped>

</style>