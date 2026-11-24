<script setup lang="ts">
import { overviewAlertPreferencesKey } from "~/composables/useAccountOverviewAlerts";
import { toggleAlertMute } from "~/utils/accountOverviewAlerts";
import type { AccountOverviewBlockId } from "~/utils/accountOverviewBlocks";

const props = defineProps<{
  playerId: string;
  blockId?: AccountOverviewBlockId;
  scopeLabel: string;
}>();
const prefs = inject(overviewAlertPreferencesKey);
const muted = computed(() =>
  props.blockId
    ? !!prefs?.value.mutedBlocks[props.playerId]?.includes(props.blockId)
    : !!prefs?.value.mutedPlayers.includes(props.playerId),
);
const inherited = computed(() => {
  if (!prefs?.value.enabled) return "Alerts are off globally.";
  if (props.blockId && prefs.value.mutedPlayers.includes(props.playerId))
    return "This player is muted.";
  return "";
});
const title = computed(() =>
  [`${muted.value ? "Unmute" : "Mute"} alerts for ${props.scopeLabel}`, inherited.value]
    .filter(Boolean)
    .join(". "),
);
</script>

<template>
  <UButton
    v-if="prefs"
    :icon="muted ? 'i-lucide-bell-off' : 'i-lucide-bell'"
    size="xs"
    color="neutral"
    :variant="muted ? 'soft' : 'ghost'"
    :class="{ 'text-muted': inherited }"
    :aria-pressed="muted"
    :aria-label="`Mute alerts for ${scopeLabel}`"
    :title="title"
    @click="toggleAlertMute(prefs, playerId, blockId)"
  />
</template>
