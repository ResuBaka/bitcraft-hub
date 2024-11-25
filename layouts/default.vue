<template>
  <v-app>
    <v-navigation-drawer v-if="$vuetify.display.mobile"
        v-model="mobileDrawer"
        location="right"
        mobile
    >
      <v-list>
        <v-list-item to="/">Leaderboards</v-list-item>
        <v-list-item to="/items">Items</v-list-item>
        <v-list-item to="/claims">Claims</v-list-item>
        <v-list-item to="/players">Players</v-list-item>
<!--        <v-list-item to="/tradeOrders">Trade orders</v-list-item>-->
        <v-list-item to="/buildings">Buildings</v-list-item>
        <v-list-item append-icon="mdi-cog-outline" @click="toggelConfigDrawer">Config</v-list-item>
      </v-list>
    </v-navigation-drawer>

    <v-app-bar>
      <v-app-bar-title>BitCraft Hub (Under construction 🚧)</v-app-bar-title>
      <v-toolbar-items v-if="!$vuetify.display.mobile">
        <v-btn to="/">Leaderboards</v-btn>
        <v-btn to="/items">Items</v-btn>
        <v-btn to="/claims">Claims</v-btn>
        <v-btn to="/players">Players</v-btn>
<!--        <v-btn to="/tradeOrders">Trade orders</v-btn>-->
        <v-btn to="/buildings">Buildings</v-btn>
        <v-btn icon="mdi-cog-outline" @click="toggelConfigDrawer"></v-btn>
      </v-toolbar-items>
      <v-toolbar-items v-if="$vuetify.display.mobile">
        <v-btn icon @click="mobileDrawer = !mobileDrawer">
          <v-icon>mdi-menu</v-icon>
        </v-btn>
      </v-toolbar-items>
    </v-app-bar>
    <v-dialog v-model="configDrawer"  width="auto">
    <v-card min-width="90vw">
      <v-card-title>Settings</v-card-title>
      <v-card-text>
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
          <v-checkbox v-model="configStore.new_api" label="Use new API"></v-checkbox>
      </v-card-text>
      <v-card-actions>
          <v-btn
              icon="mdi-close"
              variant="flat"
              @click="configDrawer = false"
          ></v-btn>
      </v-card-actions>
    </v-card>
    </v-dialog>
    <v-main>
      <NuxtPage />
    </v-main>
    <v-footer app absolute class="d-flex flex-column">
      <div class="px-4 py-2 text-center w-100">
        Not affiliated with Clockwork Labs
      </div>
    </v-footer>
  </v-app>
</template>
<script setup lang="ts">
const configStore = useConfigStore();
const configDrawer = ref(false);
const mobileDrawer = ref(false);

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
  mobileDrawer.value = false;
};
</script>