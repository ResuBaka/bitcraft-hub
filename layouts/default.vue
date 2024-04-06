<template>
  <v-app>
    <v-app-bar>
      <v-toolbar-title>Bitcraft Item Wiki (Work in Progress and Better Name wanted)</v-toolbar-title>
      <v-toolbar-items >
        <v-btn to="/">Home</v-btn>
        <v-btn to="/new">New</v-btn>
        <v-btn to="/buildings">Buildings</v-btn>
        <v-btn v-if="devmode" @click="reloadFromDisk">Reload Disk</v-btn>
      </v-toolbar-items>
    </v-app-bar>
    <v-main>
        <v-container fluid>
          <NuxtPage />
        </v-container>
    </v-main>
  </v-app>
</template>
<script setup lang="ts">
const devmode = import.meta.dev;
const reloadFromDisk = () => {
  useFetch("/api/fileReload", {
    method: "POST",
  });
  window.location.reload();
};
</script>