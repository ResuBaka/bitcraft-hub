<script setup lang="ts">
import EquipmentSlot from "./EquipmentSlot.vue";
import type { PlayerEquipmentResponse } from "~/types/PlayerEquipmentResponse";
import { equipmentView } from "~/utils/playerEquipment";

const props = defineProps<{
  equipment?: PlayerEquipmentResponse | null;
  status: "idle" | "pending" | "success" | "error";
}>();
defineEmits<{ refresh: [] }>();
const selectedId = ref<string | null>(null);
const view = computed(() => equipmentView(props.equipment, selectedId.value));
const equippedCount = computed(() => view.value.slots.filter((slot) => slot.item).length);
</script>

<template>
  <UCard :ui="{ body: 'p-4' }">
    <div class="mb-4 flex flex-wrap items-center justify-between gap-2">
      <div>
        <h2 class="text-lg font-semibold">Equipment</h2>
        <p v-if="view.slots.length" class="text-sm text-muted">
          {{ equippedCount }} of {{ view.slots.length }} slots filled
        </p>
      </div>
      <UButton
        color="neutral"
        variant="soft"
        icon="i-lucide-refresh-cw"
        label="Refresh"
        :loading="status === 'pending'"
        @click="$emit('refresh')"
      />
    </div>

    <UProgress
      v-if="status === 'idle' || (status === 'pending' && !equipment)"
      color="neutral"
      aria-label="Loading equipment"
    />
    <UEmpty
      v-else-if="status === 'error'"
      icon="i-lucide-shirt"
      title="Equipment unavailable"
      description="Could not load this player’s equipment. Try refreshing."
    />
    <template v-else>
      <div class="mb-4 space-y-2">
        <h3 class="text-sm font-medium">Equipment presets</h3>
        <div class="flex flex-wrap gap-2" role="group" aria-label="Equipment presets">
          <UButton
            v-for="loadout in view.loadouts"
            :key="loadout.id"
            :color="loadout.active ? 'success' : 'neutral'"
            :variant="view.selected.id === loadout.id ? 'outline' : 'soft'"
            :aria-pressed="view.selected.id === loadout.id"
            @click="selectedId = loadout.active ? null : loadout.id"
          >
            Slot {{ loadout.index + 1 }}
            <UBadge v-if="loadout.active" color="success" variant="subtle" size="xs">Active</UBadge>
          </UButton>
        </div>
        <p v-if="equipment?.presets == null" class="text-sm text-muted">
          Additional presets and active status are unavailable.
        </p>
      </div>
      <h3 class="mb-2 text-sm font-medium">
        {{ view.selected.active ? "Current equipment slots" : `Slot ${view.selected.index + 1} equipment` }}
      </h3>
      <ul v-if="view.slots.length" class="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
        <EquipmentSlot
          v-for="(slot, index) in view.slots"
          :key="`${slot.slot_type}:${index}`"
          :slot="slot"
        />
      </ul>
      <UEmpty v-else icon="i-lucide-shirt" title="No equipment slots recorded" />
    </template>
  </UCard>
</template>
