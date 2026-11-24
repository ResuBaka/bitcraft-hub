<script setup lang="ts">
import { alertSoundLibraryKey } from "~/composables/useAlertSoundLibrary";
import { testOverviewSoundKey } from "~/composables/useAccountOverviewAlerts";
import type { AlertType } from "~/utils/accountOverviewAlerts";

const props = defineProps<{ label: string; playerId?: string; type?: AlertType }>();
const library = inject(alertSoundLibraryKey)!;
const { clips } = library;
const testSound = inject(testOverviewSoundKey)!;
const selection = computed({
  get: () => library.selected(props.playerId, props.type),
  set: (id) => library.select(id, props.playerId, props.type),
});
</script>

<template>
  <div class="flex min-w-0 items-center gap-1">
    <select
      v-model="selection"
      :aria-label="label"
      :disabled="!clips.length"
      class="min-w-0 flex-1 rounded border border-default bg-default p-1 text-sm disabled:text-muted"
    >
      <option value="">
        {{
          type ? "Use player or global sound" : playerId ? "Use global sound" : "No default sound"
        }}
      </option>
      <option v-for="clip in clips" :key="clip.id" :value="clip.id">{{ clip.name }}</option>
    </select>
    <UButton
      icon="i-lucide-volume-2"
      size="xs"
      color="neutral"
      variant="ghost"
      :title="`Test ${label}`"
      :aria-label="`Test ${label}`"
      :disabled="!library.sound(playerId, type)"
      @click="testSound(playerId, type)"
    />
  </div>
</template>
