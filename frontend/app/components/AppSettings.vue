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
      </div>
    </template>
  </USlideover>
</template>
