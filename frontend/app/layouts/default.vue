<template>
  <v-app>
    <v-defaults-provider :defaults="vuetifyConfig">
      <ClientOnly>
        <VSonner :position="$vuetify.display.mobile ? 'bottom-center' : 'top-center'" />
      </ClientOnly>
      <v-navigation-drawer v-if="$vuetify.display.mobile"
          v-model="mobileDrawer"
          location="right"
          mobile
      >
        <v-list>
          <v-list-item to="/">Leaderboards</v-list-item>
          <v-list-item to="/items">Items</v-list-item>
          <v-list-item to="/claims">Claims</v-list-item>
          <v-list-item to="/houses">Houses</v-list-item>
          <v-list-item to="/players">Players</v-list-item>
  <!--        <v-list-item to="/tradeOrders">Trade orders</v-list-item>-->
          <v-list-item to="/buildings">Buildings</v-list-item>
          <v-list-item append-icon="mdi-cog-outline" @click="toggelConfigDrawer">Config</v-list-item>
        </v-list>
      </v-navigation-drawer>

      <v-app-bar>
        <v-app-bar-title>BitCraft Hub (Under construction 🚧)</v-app-bar-title>
        <template #append>
          <template v-if="!$vuetify.display.mobile">
            <v-btn variant="text" class="text-capitalize font-weight-black" stacked @click="reopenConnection"><v-badge dot floating :color="websocketStore.isConnected ? 'green' : 'red'">Live Data</v-badge></v-btn>
            <v-btn variant="text" class="text-capitalize font-weight-black" to="/">Leaderboards</v-btn>
            <v-btn variant="text" class="text-capitalize font-weight-black" to="/items">Items</v-btn>
            <v-btn variant="text" class="text-capitalize font-weight-black" to="/claims">Claims</v-btn>
            <v-btn variant="text" class="text-capitalize font-weight-black" to="/houses">Houses</v-btn>
            <v-btn variant="text" class="text-capitalize font-weight-black" to="/players">Players</v-btn>
            <!--        <v-btn to="/tradeOrders">Trade orders</v-btn>-->
            <v-btn variant="text" class="text-capitalize font-weight-black" to="/buildings">Buildings</v-btn>
            <v-btn variant="text" class="text-capitalize font-weight-black" icon="mdi-cog-outline" @click="toggelConfigDrawer"></v-btn>
          </template>
          <template v-else>
            <v-btn variant="text" class="text-capitalize font-weight-black" stacked @click="reopenConnection"><v-badge dot floating :color="websocketStore.isConnected ? 'green' : 'red'">Live Data</v-badge></v-btn>
            <v-btn variant="text" class="text-capitalize font-weight-black" icon @click="mobileDrawer = !mobileDrawer">
              <v-icon>mdi-menu</v-icon>
            </v-btn>
          </template>
        </template>
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
            <v-checkbox v-model="configStore.websocket.enabled_default" label="Connect to websocket by default"></v-checkbox>
            <v-checkbox v-model="configStore.show_region_as_number" label="Toggle between showing regions name or number (Default Number)"></v-checkbox>
            <v-btn variant="text" class="text-capitalize font-weight-black" @click="requestTopics">Request Listed Subscribed Topics</v-btn>
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
    </v-defaults-provider>
  </v-app>
</template>
<script setup lang="ts">
import { VSonner } from "vuetify-sonner";
import { registerWebsocketMessageHandler } from "~/composables/websocket";
const websocketStore = useWebsocketStore();
const configStore = useConfigStore();
const configDrawer = ref(false);
const mobileDrawer = ref(false);

registerWebsocketMessageHandler("SubscribedTopics", [], (message) => {
  console.log("SubscribedTopics", message);
});

const vuetifyConfig = {
  global: {
    density: "comfortable",
    size: "default",
  },
};

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

const reopenConnection = () => {
  if (websocketStore.isConnected) {
    console.log("Closing connection");
    websocketStore.close();
    return;
  }

  websocketStore.open();
};

const requestTopics = () => {
  websocketStore.sendMessage("ListSubscribedTopics");
};

const toggelConfigDrawer = () => {
  configDrawer.value = !configDrawer.value;
  mobileDrawer.value = false;
};
</script>

"inventory_insert_owner.144115188086908106"
"inventory_insert_owner.144115188086908106"