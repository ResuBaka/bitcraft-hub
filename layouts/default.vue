<template>
  <v-app>
    <v-navigation-drawer
        v-model="configDrawer"
        location="right"
        temporary
        mobile
    >
      <v-toolbar flat title="(WIP) Settings">
        <template #append>
          <v-btn icon="mdi-close" variant="flat" @click="configDrawer = false">
          </v-btn>
        </template>
      </v-toolbar>
      <v-divider />
      <v-container class="px-3 py-3">
        <v-radio-group
            v-model="configStore.theme"
            class="mb-2"
            color="primary"
            true-icon="mdi-check-circle-outline"
            hide-details
        >
          <v-radio
              v-for="(item, i) in items"
              :key="i"
              :value="item.value"
          >
            <template #label>
              <v-icon :icon="item.icon" start />

              {{ item.text }}
            </template>
          </v-radio>
        </v-radio-group>
      </v-container>
    </v-navigation-drawer>
    <v-app-bar>
      <v-toolbar-title>BitCraft Chain Base (Work in Progress and Better Name wanted)</v-toolbar-title>
      <v-toolbar-items >
        <v-btn to="/">Home</v-btn>
<!--        <v-btn to="/crafting">Crafting</v-btn>-->
        <v-btn to="/items">Items</v-btn>
        <v-btn to="/claims">Claims</v-btn>
        <v-btn to="/tradeOrders">Trade orders</v-btn>
<!--        <v-btn to="/buildings">Buildings</v-btn>-->
<!--        <v-btn to="/npcs">NPCs</v-btn>-->
<!--        <v-btn to="/professions">Professions</v-btn>-->
<!--        <v-btn v-if="devmode" @click="reloadFromDisk">Reload Disk</v-btn>-->
        <v-btn icon="mdi-cog-outline" @click="toggelConfigDrawer"></v-btn>
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
const configStore = useConfigStore();
const configDrawer = ref(false);

const items = [
  {
    text: "Light",
    icon: "mdi-white-balance-sunny",
    value: "light",
  },
  {
    text: "Dark",
    icon: "mdi-weather-night",
    value: "dark",
  },
  {
    text: "System",
    icon: "mdi-desktop-tower-monitor",
    value: "system",
  },
];

const toggelConfigDrawer = () => {
  configDrawer.value = !configDrawer.value;
};

const devmode = import.meta.dev;
const reloadFromDisk = () => {
  useFetch("/api/fileReload", {
    method: "POST",
  });
  window.location.reload();
};
</script>