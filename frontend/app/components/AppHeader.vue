<script setup lang="ts">
const items = computed(() => [
  {
    label: "Leaderboards",
    to: "/",
  },
  {
    label: "Items",
    to: "/items",
  },
  {
    label: "Claims",
    to: "/claims",
  },
  {
    label: "Players",
    to: "/players",
  },
  //   {
  //   label: 'Trade orders',
  //   to: '/tradeOrders',
  // },
  {
    label: "Buildings",
    to: "/buildings",
  },
]);

const websocketStore = useWebsocketStore();
const configStore = useConfigStore();

const websocketEnabled = computed({
  get: () => configStore.websocket.enabled_default,
  set: (enabled) => {
    console.log("enabled", enabled);
    configStore.websocket.enabled_default = enabled;
    if (enabled) {
      websocketStore.open();
    } else {
      websocketStore.close();
    }
  },
});
</script>

<template>
  <UHeader
    :ui="{
      container: 'max-w-full',
    }"
  >
    <template #left>
      <NuxtLink to="/">🚧 BitCraft Hub 🚧</NuxtLink>

      <!--      <TemplateMenu />-->
    </template>

    <template #right>
      <UNavigationMenu :items="items" variant="link" class="hidden lg:block" />

      <div class="hidden items-center gap-2 lg:flex">
        <div
          class="flex items-center gap-2 rounded-full border border-gray-200 px-3 py-1 text-xs text-gray-600 shadow-sm dark:border-gray-800 dark:text-gray-300"
        >
          <span class="uppercase tracking-[0.2em]">Websocket</span>
          <UBadge :color="websocketStore.isConnected ? 'success' : 'neutral'" variant="soft">
            {{ websocketStore.isConnected ? "Connected" : "Disconnected" }}
          </UBadge>
        </div>
        <USwitch v-model="websocketEnabled" />
      </div>
      <UColorModeSelect />
    </template>

    <template #body>
      <UNavigationMenu :items="items" orientation="vertical" class="-mx-2.5" />
      <div
        class="mt-4 flex items-center justify-between gap-2 rounded-lg border border-gray-200 px-3 py-2 text-sm text-gray-600 dark:border-gray-800 dark:text-gray-300"
      >
        <div>
          <p class="text-xs uppercase tracking-[0.2em]">Websocket</p>
          <p class="text-sm font-medium">
            {{ websocketStore.isConnected ? "Connected" : "Disconnected" }}
          </p>
        </div>
        <USwitch v-model="websocketEnabled" />
      </div>
      <UButton class="mt-4" label="Download App" variant="subtle" block />
    </template>
  </UHeader>
</template>
