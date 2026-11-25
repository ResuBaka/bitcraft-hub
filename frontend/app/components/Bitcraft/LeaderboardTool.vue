<script setup lang="ts">
import type { LeaderboardTool } from "~/types/LeaderboardTool";
import { rarityToTextClass } from "~/utils";

defineProps<{ tool?: LeaderboardTool }>();
</script>

<template>
  <NuxtLink
    v-if="tool"
    :to="{ name: 'items-type-id', params: { type: 'Item', id: tool.item_id } }"
    class="inline-flex items-center gap-2 hover:underline"
  >
    <BitcraftInventoryImg :key="tool.item_id" :item="tool" :width="24" :height="24" aria-hidden="true" />
    {{ tool.name }}
    <span v-if="tool.rarity" class="text-xs" :class="rarityToTextClass(tool.rarity)">({{ tool.rarity }})</span>
  </NuxtLink>
  <span v-else class="text-muted" aria-label="No tool available">—</span>
</template>
