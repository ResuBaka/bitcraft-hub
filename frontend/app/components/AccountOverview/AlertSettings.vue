<script setup lang="ts">
import {
  overviewAlertTypes,
  toggleAlertMute,
  type AlertPreferences,
} from "~/utils/accountOverviewAlerts";
import { accountOverviewBlocks } from "~/utils/accountOverviewBlocks";
import type { TrackedPlayer } from "~/utils/accountOverviewShare";

const props = defineProps<{ players: TrackedPlayer[] }>();
const prefs = defineModel<AlertPreferences>({ required: true });
const selected = ref("");
const alertBlocks = accountOverviewBlocks.filter((block) =>
  Object.values(overviewAlertTypes).some((type) => type.block === block.id),
);
const player = computed(
  () => props.players.find((p) => p.id === selected.value) ?? props.players[0],
);
</script>

<template>
  <UModal
    title="Account alerts"
    description="Choose what to watch for on your dashboard. Enable live updates to receive changes as they happen."
  >
    <UButton
      label="Alerts"
      :icon="prefs.enabled ? 'i-lucide-bell' : 'i-lucide-bell-off'"
      color="neutral"
      variant="soft"
    />
    <template #body>
      <div class="space-y-4">
        <USwitch
          v-model="prefs.enabled"
          label="Enable alerts"
          description="Turning this off mutes all players and sounds."
        />
        <AccountOverviewAlertSoundLibrary />
        <div>
          <p class="mb-1 text-sm font-medium">Global sound</p>
          <AccountOverviewAlertSound label="Global sound" />
        </div>
        <fieldset class="space-y-2">
          <legend class="mb-1 text-sm font-medium">Alert types</legend>
          <p class="text-xs text-muted">
            A sound selected for an alert type overrides the player and global sounds.
          </p>
          <div
            v-for="(type, id) in overviewAlertTypes"
            :key="id"
            class="grid items-center gap-1 sm:grid-cols-2"
          >
            <label class="flex items-center gap-2 text-sm">
              <input v-model="prefs.enabledTypes" type="checkbox" :value="id" />
              <span
                >{{ type.label }}
                <span v-if="id === 'full-stamina'" class="text-xs text-muted">
                  (except while crafting, gathering, or a stamina regeneration buff is active)
                </span>
                <span v-else-if="id === 'food-expired'" class="text-xs text-muted">
                  (only buffs with a total duration over 1 minute)
                </span>
                <span v-else-if="id === 'activity-stopped'" class="text-xs text-muted">
                  (if stamina ran out, the timer starts when stamina is full again)
                </span>
              </span>
            </label>
            <AccountOverviewAlertSound
              :label="`${type.label} sound`"
              :type="id"
              :player-id="player?.id"
            />
          </div>
        </fieldset>
        <div class="space-y-1">
          <label for="alert-volume" class="text-sm font-medium">Alert volume</label>
          <div class="flex items-center gap-3">
            <input
              id="alert-volume"
              v-model.number="prefs.volume"
              type="range"
              min="0"
              max="100"
              :aria-valuetext="`${prefs.volume}%`"
              class="min-w-0 flex-1 accent-primary"
            />
            <span class="w-10 text-right text-sm tabular-nums">{{ prefs.volume }}%</span>
          </div>
          <p class="text-xs text-muted">
            Applies to all alert sounds. At 0%, visual alerts still appear.
          </p>
        </div>
        <section v-if="player" class="space-y-3 border-t border-default pt-3">
          <UFormField label="Player settings">
            <USelect
              :model-value="player.id"
              :items="players"
              value-key="id"
              label-key="name"
              class="w-full"
              @update:model-value="selected = $event"
            />
          </UFormField>
          <label class="flex items-center gap-2 text-sm font-medium">
            <input v-model="prefs.mutedPlayers" type="checkbox" :value="player.id" />
            Mute all alerts for {{ player.name }}
          </label>
          <fieldset class="flex flex-wrap gap-x-4 gap-y-1">
            <legend class="mb-1 text-sm font-medium">
              Mute individual blocks for {{ player.name }}
            </legend>
            <label
              v-for="block in alertBlocks"
              :key="block.id"
              class="flex items-center gap-2 text-sm"
            >
              <input
                type="checkbox"
                :checked="prefs.mutedBlocks[player.id]?.includes(block.id)"
                @change="toggleAlertMute(prefs, player.id, block.id)"
              />
              {{ block.name }}
            </label>
          </fieldset>
          <div>
            <p class="mb-1 text-sm font-medium">Player sound</p>
            <AccountOverviewAlertSound label="Player sound" :player-id="player.id" />
          </div>
        </section>
      </div>
    </template>
  </UModal>
</template>
