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

registerWebsocketMessageHandler("SubscribedTopics", [], (message) => {
  console.log("SubscribedTopics", message);
});
</script>
