<script setup lang="ts">
import type { ItemsAndCargollResponse } from "~/types/ItemsAndCargollResponse";

defineProps<{
  travelerColumns: { id: string; header: string }[];
  travelerTaskRows: {
    players: bigint[];
    items: { item_type: "Item" | "Cargo"; item_id: number; quantity: number }[];
    npc_name: string;
    player_count: number;
  }[];
  claimMembers: Record<string, { user_name: string }>;
  itemsAndCargoAllFetch?: ItemsAndCargollResponse;
  tierToColor: Record<number, string>;
  numberFormat: Intl.NumberFormat;
  getTravelerItemIcon: (shownItem: {
    item_type: "Item" | "Cargo";
    item_id: number;
  }) => string | null;
}>();
</script>

<template>
  <div class="flex flex-col gap-4">
    <UTable :columns="travelerColumns" :data="travelerTaskRows" class="claim-table">
      <template #items-cell="{ row }">
        <div class="flex flex-wrap gap-2">
          <div
            v-for="shownItem of row.original.items"
            :key="`${shownItem.item_type}-${shownItem.item_id}`"
            class="flex items-center gap-2"
          >
            <div class="relative">
              <img
                v-if="getTravelerItemIcon(shownItem)"
                :src="getTravelerItemIcon(shownItem) || ''"
                alt=""
                class="h-10 w-10 rounded-md object-contain"
              />
              <span
                class="absolute -right-2 -top-2 rounded-full bg-gray-900 px-1 text-[10px] font-semibold text-white dark:bg-gray-100 dark:text-gray-900"
              >
                {{ numberFormat.format(shownItem.quantity) }}
              </span>
            </div>
          </div>
        </div>
      </template>
      <template #name-cell="{ row }">
        <div class="space-y-1">
          <div
            v-for="shownItem of row.original.items"
            :key="`${shownItem.item_type}-${shownItem.item_id}-name`"
            :class="
              tierToColor[
                shownItem.item_type === 'Item'
                  ? itemsAndCargoAllFetch?.item_desc?.[shownItem.item_id]?.tier
                  : itemsAndCargoAllFetch?.cargo_desc?.[shownItem.item_id]?.tier
              ]
            "
          >
            {{
              shownItem.item_type === "Item"
                ? itemsAndCargoAllFetch?.item_desc?.[shownItem.item_id]?.name
                : itemsAndCargoAllFetch?.cargo_desc?.[shownItem.item_id]?.name
            }}
          </div>
        </div>
      </template>
      <template #npc_name-cell="{ row }">{{ row.original.npc_name }}</template>
      <template #player_count-cell="{ row }">{{ row.original.player_count }}</template>
      <template #users-cell="{ row }">
        <div class="flex flex-wrap gap-2">
          <NuxtLink
            v-for="playerId of row.original.players"
            :key="playerId"
            :to="{ name: 'players-id', params: { id: playerId } }"
            class="text-sm font-semibold text-gray-900 hover:underline dark:text-gray-100"
          >
            {{ claimMembers[playerId.toString()]?.user_name }}
          </NuxtLink>
        </div>
      </template>
    </UTable>
  </div>
</template>

<style scoped>
.claim-table :deep(thead tr th) {
  font-size: 0.75rem;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: rgba(100, 116, 139, 0.9);
}
.claim-table :deep(tbody tr td) {
  vertical-align: top;
}
</style>
