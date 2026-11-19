<script setup lang="ts">
import {
  accountOverviewBlocks,
  blockDragType,
  type AccountOverviewBlockId,
} from "~/utils/accountOverviewBlocks";

const props = defineProps<{ players: { id: string; name: string }[] }>();
const emit = defineEmits<{ add: [playerId: string, blockId: AccountOverviewBlockId] }>();
const targetPlayer = ref("");

watch(
  () => props.players.map((player) => player.id),
  (ids) => {
    if (!ids.includes(targetPlayer.value)) targetPlayer.value = ids[0] ?? "";
  },
  { immediate: true },
);

function startDrag(event: DragEvent, id: AccountOverviewBlockId) {
  if (!event.dataTransfer) return;
  event.dataTransfer.setData(blockDragType, id);
  event.dataTransfer.effectAllowed = "copy";
}
</script>

<template>
  <aside
    class="h-fit rounded-lg border border-default p-3 lg:sticky lg:top-4"
    aria-labelledby="available-blocks"
  >
    <h2 id="available-blocks" class="font-semibold">Information Blocks</h2>
    <p class="mt-1 text-xs text-muted">
      Drag a block into a player box, or select a player and add it below. Each player can have one
      of each block.
    </p>
    <label class="mt-3 block text-sm">
      <span>Player</span>
      <select
        v-model="targetPlayer"
        class="mt-1 w-full rounded border border-default bg-default px-2 py-1.5"
        :disabled="!players.length"
      >
        <option v-if="!players.length" value="">Add a player first</option>
        <option v-for="player in players" :key="player.id" :value="player.id">
          {{ player.name }}
        </option>
      </select>
    </label>
    <ul class="mt-3 space-y-2">
      <li
        v-for="block in accountOverviewBlocks"
        :key="block.id"
        class="flex items-center gap-2 rounded border border-default p-2"
      >
        <div
          draggable="true"
          class="flex min-w-0 flex-1 cursor-grab items-center gap-2 active:cursor-grabbing"
          :title="`Drag ${block.name} into a player box`"
          @dragstart="startDrag($event, block.id)"
        >
          <UIcon :name="block.icon" class="size-4 shrink-0 text-muted" />
          <div class="min-w-0">
            <p class="text-sm font-medium">{{ block.name }}</p>
            <p class="text-xs text-muted">{{ block.description }}</p>
          </div>
        </div>
        <UButton
          icon="i-lucide-plus"
          size="xs"
          color="neutral"
          variant="ghost"
          :disabled="!targetPlayer"
          :aria-label="`Add ${block.name} to selected player`"
          @click="emit('add', targetPlayer, block.id)"
        />
      </li>
    </ul>
  </aside>
</template>
