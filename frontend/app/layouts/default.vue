<template>
  <u-app>
    <AppHeader />
    <UMain>
      <slot />
    </UMain>
    <AppFooter />
  </u-app>
</template>
<script setup lang="ts">
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
