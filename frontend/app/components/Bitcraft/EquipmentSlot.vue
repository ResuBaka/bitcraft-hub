<script setup lang="ts">
import type { PlayerEquipmentSlot } from "~/types/PlayerEquipmentSlot";
import { equipmentSlotLabel } from "~/utils/playerEquipment";

defineProps<{ slot: PlayerEquipmentSlot; label?: string }>();
</script>

<template>
  <li
    class="flex items-center gap-2 rounded-lg border p-2"
    :class="slot.item ? 'border-default' : 'border-dashed border-muted'"
  >
    <BitcraftInventoryImg
      v-if="slot.item"
      :key="`${slot.item.item_type}:${slot.item.item_id}`"
      :item="{
        name: slot.item.name ?? '',
        icon_asset_name: slot.item.icon_asset_name ?? '',
      }"
      :width="32"
      :height="32"
      class="size-8 shrink-0"
    />
    <div v-else class="grid size-8 shrink-0 place-items-center rounded bg-muted text-muted">
      <UIcon name="i-lucide-square" aria-hidden="true" />
    </div>
    <div class="min-w-0">
      <p class="text-xs font-medium text-muted">{{ label ?? equipmentSlotLabel(slot) }}</p>
      <NuxtLink
        v-if="slot.item?.name"
        :to="{
          name: 'items-type-id',
          params: { id: slot.item.item_id, type: slot.item.item_type },
        }"
        class="break-words text-sm font-semibold hover:underline"
      >
        {{ slot.item.name }}
      </NuxtLink>
      <p v-else-if="slot.item" class="text-sm">Unknown equipment ({{ slot.item.item_id }})</p>
      <p v-else class="text-sm text-muted">Empty</p>
    </div>
  </li>
</template>
