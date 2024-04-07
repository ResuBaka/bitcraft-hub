<script setup lang="ts">
import type {Profession} from "~/types";

const { data, refresh, pending } = useFetch<Profession[]>("/api/professions");

const professions = computed(() => {
  return data.value;
});

const search = ref("");

const filteredItems = computed(() => {
  let items = professions.value;

  if (search.value) {
    items = professions.value.filter((item) =>
        item.id.toLowerCase().includes(search.value.toLowerCase()),
    );
  }

  return items;
});

const deleteProfession = async (profession : Profession) => {
  console.log(profession);
  const { status } = await useFetch(`/api/professions/${profession.id}`, {
    method: "DELETE",
  });

  if (status.value !== "success") {
    console.log("An error happened while deleting profession");
    return;
  }

  refresh();
};
</script>

<template>
  <v-container>
    <v-row>
      <v-col cols="10">
        <v-text-field v-model="search"></v-text-field>
      </v-col>
      <v-col cols="2">
        <v-btn to="/professions/new">New Profession</v-btn>
      </v-col>
    </v-row>
    <v-row>
      <v-col v-for="profession in filteredItems" :key="profession.id" cols="6" md="4">
        <v-card width="200">
          <v-card-title><nuxt-link :to="{ name: 'professions-id', params: { id: profession.id } }">{{ profession.id }}</nuxt-link></v-card-title>
          <v-btn @click="deleteProfession(profession)">Delete</v-btn>
        </v-card>
<!--        <profession @click="deleteProfessionConfirm" :profession="profession"></profession>-->
      </v-col>
    </v-row>
  </v-container>
</template>
