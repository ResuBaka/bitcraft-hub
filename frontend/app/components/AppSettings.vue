<script setup lang="ts">
const configStore = useConfigStore();
const websocketStore = useWebsocketStore();
const colorMode = useColorMode();

// Keep the config in sync with the existing appearance selector as well.
watch(
  () => colorMode.preference,
  (preference) => {
    configStore.theme = preference;
  },
);
onMounted(() => {
  configStore.theme = colorMode.preference;
});

// Claim links still use a central selector; player names use BitcraftPlayerName.
useHead(() => {
  const selectors = [
    ...configStore.highlighted_claims.flatMap(({ id }) =>
      /^\d+$/.test(id) ? [`a[href="/claims/${id}"]`] : [],
    ),
  ];
  return {
    style: selectors.length
      ? [{
          key: "highlighted-entities",
          innerHTML: `${selectors.map((selector) => `${selector}::after`).join(",")} { content: ""; display: inline-block; width: 0.85em; height: 0.85em; margin-left: 0.35em; vertical-align: -0.05em; background-color: #f59e0b; mask: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M11.048 2.927a1 1 0 0 1 1.904 0l1.313 4.038a2 2 0 0 0 1.902 1.382h4.246a1 1 0 0 1 .588 1.81l-3.435 2.496a2 2 0 0 0-.727 2.236l1.312 4.038a1 1 0 0 1-1.538 1.118l-3.435-2.496a2 2 0 0 0-2.351 0l-3.435 2.496a1 1 0 0 1-1.539-1.118l1.313-4.038a2 2 0 0 0-.726-2.236L2.995 10.16a1 1 0 0 1 .588-1.81h4.245A2 2 0 0 0 9.73 6.966z'/%3E%3C/svg%3E") center / contain no-repeat; }`,
        }]
      : [],
  };
});
</script>

<template>
  <USlideover title="Settings" description="Preferences are saved automatically in this browser.">
    <UButton
      icon="i-lucide-settings"
      color="neutral"
      variant="ghost"
      aria-label="Open settings"
      title="Settings"
    />

    <template #body>
      <div class="space-y-8">
        <section class="space-y-4" aria-labelledby="settings-display">
          <h2 id="settings-display" class="text-sm font-semibold text-highlighted">Display</h2>
          <UFormField label="Appearance" name="appearance">
            <UColorModeSelect id="appearance" class="w-full" />
          </UFormField>
          <USwitch
            v-model="configStore.show_region_as_number"
            label="Show region numbers"
            description="Display region numbers instead of region names throughout the site."
          />
        </section>

        <section class="space-y-4" aria-labelledby="settings-live-updates">
          <div class="flex items-center justify-between gap-3">
            <h2 id="settings-live-updates" class="text-sm font-semibold text-highlighted">
              Live updates
            </h2>
            <UBadge :color="websocketStore.isConnected ? 'success' : 'neutral'" variant="soft">
              {{ websocketStore.isConnected ? "Connected" : "Disconnected" }}
            </UBadge>
          </div>
          <USwitch
            v-model="configStore.websocket.enabled_default"
            label="Enable live updates"
            description="Receive updates over WebSocket without refreshing the page. Also applies on future visits."
          />
        </section>
        <AppHighlightSettings />
      </div>
    </template>
  </USlideover>
</template>
